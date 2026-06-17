"""
Rate limiter for Anthropic API — semaphore-based concurrency control
with sliding-window token budget tracking.

Designed for ~50 concurrent users sharing Tier 2/3 rate limits.
"""
import asyncio
import time
from dataclasses import dataclass, field
from enum import IntEnum
from typing import Optional

from .config import settings


class Priority(IntEnum):
    """Request priority — lower value = higher priority."""
    LIGHT = 0   # summary, bibliography, review
    HEAVY = 1   # content generation


@dataclass
class _UsageRecord:
    timestamp: float
    input_tokens: int
    output_tokens: int


@dataclass
class TierConfig:
    """Anthropic tier rate-limit configuration."""
    rpm: int                 # requests per minute
    input_tpm: int           # input tokens per minute
    output_tpm: int          # output tokens per minute (OTPM — the real bottleneck)
    max_concurrent_heavy: int
    max_concurrent_light: int

    # Per-user caps (prevent monopolization)
    per_user_heavy: int = 1
    per_user_light: int = 2


TIER_CONFIGS: dict[int, TierConfig] = {
    2: TierConfig(
        rpm=2000,
        input_tpm=200_000,
        output_tpm=90_000,
        max_concurrent_heavy=1,
        max_concurrent_light=3,
    ),
    3: TierConfig(
        rpm=4000,
        input_tpm=400_000,
        output_tpm=160_000,
        max_concurrent_heavy=2,
        max_concurrent_light=5,
    ),
    4: TierConfig(
        rpm=4000,
        input_tpm=400_000,
        output_tpm=400_000,
        max_concurrent_heavy=4,
        max_concurrent_light=8,
    ),
}

# Safety margin — only use 85% of the actual limit
_SAFETY_FACTOR = 0.85
_WINDOW_SECONDS = 60.0


class TokenBudgetTracker:
    """Sliding 60-second window tracking actual token usage."""

    def __init__(self, tier_config: TierConfig) -> None:
        self._config = tier_config
        self._records: list[_UsageRecord] = []
        self._lock = asyncio.Lock()

    def _prune(self, now: float) -> None:
        cutoff = now - _WINDOW_SECONDS
        self._records = [r for r in self._records if r.timestamp > cutoff]

    def _current_output_tokens(self, now: float) -> int:
        self._prune(now)
        return sum(r.output_tokens for r in self._records)

    def _current_input_tokens(self, now: float) -> int:
        self._prune(now)
        return sum(r.input_tokens for r in self._records)

    def _current_requests(self, now: float) -> int:
        self._prune(now)
        return len(self._records)

    async def record_usage(self, input_tokens: int, output_tokens: int) -> None:
        async with self._lock:
            self._records.append(_UsageRecord(
                timestamp=time.monotonic(),
                input_tokens=input_tokens,
                output_tokens=output_tokens,
            ))

    async def can_proceed(self, estimated_output: int) -> bool:
        async with self._lock:
            now = time.monotonic()
            effective_otpm = int(self._config.output_tpm * _SAFETY_FACTOR)
            current = self._current_output_tokens(now)
            return (current + estimated_output) <= effective_otpm

    async def estimated_wait_seconds(self, estimated_output: int) -> float:
        """Estimate how long until there's enough OTPM budget."""
        async with self._lock:
            now = time.monotonic()
            effective_otpm = int(self._config.output_tpm * _SAFETY_FACTOR)
            current = self._current_output_tokens(now)
            needed = (current + estimated_output) - effective_otpm
            if needed <= 0:
                return 0.0

            # Find when enough tokens expire from the window
            cutoff = now - _WINDOW_SECONDS
            accumulated = 0
            for record in sorted(self._records, key=lambda r: r.timestamp):
                if record.timestamp <= cutoff:
                    continue
                accumulated += record.output_tokens
                if accumulated >= needed:
                    # This record will expire at record.timestamp + WINDOW
                    wait = (record.timestamp + _WINDOW_SECONDS) - now
                    return max(0.0, wait)

            # Worst case: wait for the full window to roll over
            return _WINDOW_SECONDS

    async def get_status(self) -> dict:
        async with self._lock:
            now = time.monotonic()
            return {
                "output_tokens_used": self._current_output_tokens(now),
                "output_tokens_limit": int(self._config.output_tpm * _SAFETY_FACTOR),
                "input_tokens_used": self._current_input_tokens(now),
                "requests_in_window": self._current_requests(now),
            }


@dataclass
class _QueueEntry:
    """A waiting request in the semaphore queue."""
    user_id: str
    priority: Priority
    event: asyncio.Event = field(default_factory=asyncio.Event)
    position: int = 0
    # True only when a slot has been pre-acquired for this queued entry in _wake_next.
    slot_acquired: bool = False


class PrioritySemaphore:
    """Async concurrency limiter with priority and per-user fairness."""

    def __init__(self, tier_config: TierConfig) -> None:
        self._config = tier_config
        self._lock = asyncio.Lock()

        # Active slot counts
        self._active_heavy: int = 0
        self._active_light: int = 0

        # Per-user active counts: user_id -> {HEAVY: count, LIGHT: count}
        self._user_slots: dict[str, dict[Priority, int]] = {}

        # Waiting queue (sorted by priority then insertion order)
        self._queue: list[_QueueEntry] = []

    def _user_count(self, user_id: str, priority: Priority) -> int:
        return self._user_slots.get(user_id, {}).get(priority, 0)

    def _can_acquire(self, user_id: str, priority: Priority) -> bool:
        # Check global limits
        if priority == Priority.HEAVY:
            if self._active_heavy >= self._config.max_concurrent_heavy:
                return False
            if self._user_count(user_id, Priority.HEAVY) >= self._config.per_user_heavy:
                return False
        else:
            if self._active_light >= self._config.max_concurrent_light:
                return False
            if self._user_count(user_id, Priority.LIGHT) >= self._config.per_user_light:
                return False
        return True

    def _acquire_slot(self, user_id: str, priority: Priority) -> None:
        if priority == Priority.HEAVY:
            self._active_heavy += 1
        else:
            self._active_light += 1

        if user_id not in self._user_slots:
            self._user_slots[user_id] = {Priority.HEAVY: 0, Priority.LIGHT: 0}
        self._user_slots[user_id][priority] += 1

    def _release_slot(self, user_id: str, priority: Priority) -> None:
        if priority == Priority.HEAVY:
            self._active_heavy = max(0, self._active_heavy - 1)
        else:
            self._active_light = max(0, self._active_light - 1)

        if user_id in self._user_slots:
            self._user_slots[user_id][priority] = max(
                0, self._user_slots[user_id][priority] - 1
            )
            # Clean up empty entries
            if all(v == 0 for v in self._user_slots[user_id].values()):
                del self._user_slots[user_id]

    def _update_positions(self) -> None:
        for i, entry in enumerate(self._queue):
            entry.position = i + 1

    async def acquire(self, user_id: str, priority: Priority) -> int:
        """Acquire a slot. Returns queue position (0 = immediate, >0 = waiting)."""
        entry: Optional[_QueueEntry] = None
        async with self._lock:
            if self._can_acquire(user_id, priority):
                self._acquire_slot(user_id, priority)
                return 0

            # Add to queue
            entry = _QueueEntry(user_id=user_id, priority=priority)
            self._queue.append(entry)
            # Sort: LIGHT before HEAVY (lower priority value first)
            self._queue.sort(key=lambda e: e.priority)
            self._update_positions()
            position = entry.position

        # Wait outside lock. If caller is cancelled, clean queue/slot state.
        try:
            await entry.event.wait()
            return position
        except asyncio.CancelledError:
            async with self._lock:
                if entry in self._queue:
                    self._queue.remove(entry)
                    self._update_positions()
                elif entry.slot_acquired:
                    # _wake_next may have pre-acquired a slot for this entry.
                    # Release it to avoid leaked active slots on disconnect/cancel.
                    self._release_slot(entry.user_id, entry.priority)
                    self._wake_next()
            raise

    async def release(self, user_id: str, priority: Priority) -> None:
        async with self._lock:
            self._release_slot(user_id, priority)
            self._wake_next()

    def _wake_next(self) -> None:
        """Wake the next eligible queued entry (must be called under lock)."""
        for i, entry in enumerate(self._queue):
            if self._can_acquire(entry.user_id, entry.priority):
                self._acquire_slot(entry.user_id, entry.priority)
                entry.slot_acquired = True
                self._queue.pop(i)
                self._update_positions()
                entry.event.set()
                return

    async def get_queue_position(self, entry: _QueueEntry) -> int:
        async with self._lock:
            try:
                return self._queue.index(entry) + 1
            except ValueError:
                return 0

    async def get_status(self) -> dict:
        async with self._lock:
            return {
                "active_heavy": self._active_heavy,
                "max_heavy": self._config.max_concurrent_heavy,
                "active_light": self._active_light,
                "max_light": self._config.max_concurrent_light,
                "queue_length": len(self._queue),
                "queue_heavy": sum(1 for e in self._queue if e.priority == Priority.HEAVY),
                "queue_light": sum(1 for e in self._queue if e.priority == Priority.LIGHT),
            }


class RateLimiter:
    """Facade combining semaphore concurrency control and token budget tracking.

    Usage:
        async with rate_limiter.throttle(user_id, Priority.HEAVY, 30000) as slot:
            # slot.queue_position is the initial position (0 = immediate)
            # slot.estimated_wait is seconds estimate
            ...do API call...
        # On exit, slot is released automatically
    """

    def __init__(self, tier: Optional[int] = None) -> None:
        tier_num = tier or settings.anthropic_tier
        if tier_num not in TIER_CONFIGS:
            raise ValueError(f"Unknown tier {tier_num}. Supported: {list(TIER_CONFIGS.keys())}")

        self.tier = tier_num
        self.config = TIER_CONFIGS[tier_num]
        self.tracker = TokenBudgetTracker(self.config)
        self.semaphore = PrioritySemaphore(self.config)

    def throttle(
        self,
        user_id: str,
        priority: Priority,
        estimated_output: int = 0,
    ) -> "_ThrottleContext":
        return _ThrottleContext(self, user_id, priority, estimated_output)

    async def get_status(self) -> dict:
        token_status = await self.tracker.get_status()
        semaphore_status = await self.semaphore.get_status()
        return {
            "tier": self.tier,
            "tokens": token_status,
            "semaphore": semaphore_status,
        }


class _ThrottleContext:
    """Async context manager for rate-limited API calls."""

    def __init__(
        self,
        limiter: RateLimiter,
        user_id: str,
        priority: Priority,
        estimated_output: int,
    ) -> None:
        self._limiter = limiter
        self._user_id = user_id
        self._priority = priority
        self._estimated_output = estimated_output
        self.queue_position: int = 0
        self.estimated_wait: float = 0.0
        self._slot_acquired: bool = False

    async def __aenter__(self) -> "_ThrottleContext":
        try:
            # 1. Acquire semaphore slot (may queue)
            self.queue_position = await self._limiter.semaphore.acquire(
                self._user_id, self._priority
            )
            self._slot_acquired = True

            # 2. Wait for token budget if needed
            if self._estimated_output > 0:
                while not await self._limiter.tracker.can_proceed(self._estimated_output):
                    wait = await self._limiter.tracker.estimated_wait_seconds(
                        self._estimated_output
                    )
                    self.estimated_wait = wait
                    await asyncio.sleep(min(wait, 5.0))  # recheck every 5s max

            return self
        except asyncio.CancelledError:
            if self._slot_acquired:
                await self._limiter.semaphore.release(self._user_id, self._priority)
                self._slot_acquired = False
            raise
        except Exception:
            if self._slot_acquired:
                await self._limiter.semaphore.release(self._user_id, self._priority)
                self._slot_acquired = False
            raise

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        if self._slot_acquired:
            await self._limiter.semaphore.release(self._user_id, self._priority)
            self._slot_acquired = False
        return None


# Singleton
_rate_limiter: Optional[RateLimiter] = None


def get_rate_limiter() -> RateLimiter:
    global _rate_limiter
    if _rate_limiter is None:
        _rate_limiter = RateLimiter()
    return _rate_limiter


def init_rate_limiter(tier: Optional[int] = None) -> RateLimiter:
    """Initialize the singleton (call at startup)."""
    global _rate_limiter
    _rate_limiter = RateLimiter(tier=tier)
    return _rate_limiter
