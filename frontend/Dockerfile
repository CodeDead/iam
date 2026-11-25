# ---- Builder Stage ----
FROM node:26-alpine AS builder

WORKDIR /app

COPY package.json yarn.lock .yarnrc.yml ./
COPY .yarn ./.yarn
RUN --mount=type=cache,target=/root/.yarn/berry/cache \
    node .yarn/releases/yarn-4.19.0.cjs install --immutable

ARG VITE_API_BASE_URL=/api/v1
ENV VITE_API_BASE_URL=${VITE_API_BASE_URL}

COPY index.html vite.config.js postcss.config.cjs ./
COPY public ./public
COPY src ./src
COPY security-headers.inc ./security-headers.inc

RUN node .yarn/releases/yarn-4.19.0.cjs build && node -e ' \
  const fs = require("fs"); \
  const raw = (process.env.VITE_API_BASE_URL || "").trim(); \
  let extra = ""; \
  if (/^https?:\/\//i.test(raw)) { \
    try { extra = " " + new URL(raw).origin; } catch {} \
  } \
  const tpl = fs.readFileSync("security-headers.inc", "utf8"); \
  fs.writeFileSync("security-headers.rendered.inc", tpl.replace("__CSP_CONNECT_EXTRA__", extra)); \
'

# ---- Runtime Stage (Non-Root Unprivileged Nginx) ----
FROM nginxinc/nginx-unprivileged:stable-alpine AS runtime

COPY --from=builder /app/security-headers.rendered.inc /etc/nginx/conf.d/security-headers.inc
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD wget -qO- http://127.0.0.1:8080/healthz || exit 1

CMD ["nginx", "-g", "daemon off;"]
