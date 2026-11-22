# ---- Builder Stage ----
FROM node:26-alpine AS builder

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

ARG VITE_API_BASE_URL=http://localhost:8080/api/v1
ENV VITE_API_BASE_URL=${VITE_API_BASE_URL}

COPY index.html vite.config.js postcss.config.cjs ./
COPY public ./public
COPY src ./src

RUN npm run build

# ---- Runtime Stage (Non-Root Unprivileged Nginx) ----
FROM nginxinc/nginx-unprivileged:stable-alpine AS runtime

COPY security-headers.inc /etc/nginx/conf.d/security-headers.inc
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD wget -qO- http://127.0.0.1:8080/healthz || exit 1

CMD ["nginx", "-g", "daemon off;"]
