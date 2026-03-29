const CACHE_NAME = 'aura-alpha-v5';
const STATIC_ASSETS = [
  '/',
  '/index.html',
  '/manifest.json',
];

// Critical API endpoints that should be cached for offline fallback.
// When the API is unreachable, users see "last known data" instead of blank pages.
const CRITICAL_API_PATHS = [
  '/api/telemetry/latest',
  '/api/strategies',
  '/api/system/health',
  '/api/health',
  '/api/control/health',
  '/api/positions',
];

function isCriticalApiPath(pathname) {
  return CRITICAL_API_PATHS.some(p => pathname === p || pathname.startsWith(p + '?'));
}

// Install: precache app shell
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(STATIC_ASSETS))
  );
  self.skipWaiting();
});

// Activate: clean old caches
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE_NAME).map((k) => caches.delete(k)))
    )
  );
  self.clients.claim();
});

// Fetch: network-first for API, cache-first for static assets
self.addEventListener('fetch', (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // Skip non-GET requests
  if (request.method !== 'GET') return;

  // ── Critical API endpoints: network-first with offline fallback ──
  // These get special treatment: always try network first, cache response on success,
  // serve cached response with an X-Aura-Cached header when offline so the UI
  // can show "Last known data" indicators.
  if (url.pathname.startsWith('/api/') && isCriticalApiPath(url.pathname)) {
    event.respondWith(
      fetch(request)
        .then((response) => {
          if (response.ok) {
            const clone = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(request, clone));
          }
          return response;
        })
        .catch(async () => {
          const cache = await caches.open(CACHE_NAME);
          const cached = await cache.match(request);
          if (cached) {
            // Clone the cached response and add a header to indicate it's from cache
            const headers = new Headers(cached.headers);
            headers.set('X-Aura-Cached', 'true');
            headers.set('X-Aura-Cache-Time', cached.headers.get('date') || 'unknown');
            return new Response(cached.body, {
              status: cached.status,
              statusText: cached.statusText,
              headers,
            });
          }
          // No cache available, return a structured error JSON
          return new Response(
            JSON.stringify({ error: 'offline', message: 'No cached data available' }),
            { status: 503, headers: { 'Content-Type': 'application/json' } }
          );
        })
    );
    return;
  }

  // ── Non-critical API calls: stale-while-revalidate ──
  if (url.pathname.startsWith('/api/')) {
    event.respondWith(
      caches.open(CACHE_NAME).then(async (cache) => {
        const cached = await cache.match(request);
        const fetchPromise = fetch(request)
          .then((response) => {
            if (response.ok) {
              cache.put(request, response.clone());
            }
            return response;
          })
          .catch(() => cached);
        return cached || fetchPromise;
      })
    );
    return;
  }

  // Static assets: cache-first, fallback to network
  if (url.pathname.startsWith('/assets/') || url.pathname.endsWith('.js') || url.pathname.endsWith('.css')) {
    event.respondWith(
      caches.match(request).then((cached) => {
        if (cached) return cached;
        return fetch(request).then((response) => {
          if (response.ok) {
            const clone = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(request, clone));
          }
          return response;
        });
      })
    );
    return;
  }

  // HTML navigation: network-first with fallback to cached index
  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request)
        .then((response) => {
          const clone = response.clone();
          caches.open(CACHE_NAME).then((cache) => cache.put(request, clone));
          return response;
        })
        .catch(() => caches.match('/index.html'))
    );
    return;
  }
});

// Listen for messages (e.g., skip waiting from update prompt)
self.addEventListener('message', (event) => {
  if (event.data === 'skipWaiting') {
    self.skipWaiting();
  }
});
