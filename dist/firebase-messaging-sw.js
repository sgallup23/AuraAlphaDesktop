/* Firebase Cloud Messaging service worker for background push notifications.
 *
 * This file must live at /firebase-messaging-sw.js (public root) so the
 * Firebase SDK can register it with the correct scope.
 *
 * Firebase config values are baked at build/deploy time.  Replace the
 * placeholders below with your Aura Alpha Firebase project values,
 * or inject via CI before deploying.
 */

importScripts('https://www.gstatic.com/firebasejs/10.12.0/firebase-app-compat.js');
importScripts('https://www.gstatic.com/firebasejs/10.12.0/firebase-messaging-compat.js');

firebase.initializeApp({
  apiKey:            'AIzaSyCn_Drp5ZqJCubfAAe98gc6i5nw2NGf-4Q',
  authDomain:        'aura-alpha-dcd14.firebaseapp.com',
  projectId:         'aura-alpha-dcd14',
  storageBucket:     'aura-alpha-dcd14.firebasestorage.app',
  messagingSenderId: '631554103305',
  appId:             '1:631554103305:web:8b829cad8231962e5e00d8',
});

const messaging = firebase.messaging();

messaging.onBackgroundMessage((payload) => {
  const title = payload.notification?.title || 'Aura Alpha';
  const options = {
    body: payload.notification?.body || '',
    icon: '/aura-icon-192.png',
    badge: '/aura-icon-192.png',
    data: payload.data || {},
  };
  self.registration.showNotification(title, options);
});

// Open app when notification is clicked
self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const url = event.notification.data?.url || '/';
  event.waitUntil(
    clients.matchAll({ type: 'window', includeUncontrolled: true }).then((windowClients) => {
      for (const client of windowClients) {
        if (client.url.includes(self.location.origin) && 'focus' in client) {
          return client.focus();
        }
      }
      return clients.openWindow(url);
    })
  );
});
