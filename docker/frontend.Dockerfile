FROM node:22-alpine AS build

WORKDIR /app/web

COPY web/package*.json ./
RUN npm ci

COPY web/ ./

ARG VITE_API_URL=/e-learning
ARG VITE_BASE_PATH=/e-learning/
ENV VITE_API_URL=${VITE_API_URL} \
    VITE_BASE_PATH=${VITE_BASE_PATH}

RUN npm run build

FROM nginx:1.27-alpine

COPY docker/frontend-nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/web/dist /usr/share/nginx/html

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1/healthz || exit 1
