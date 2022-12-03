import './sphere-viewer.js';

import { openSphere, connectToNoosphere, openFile } from './state/actions.js';
import { store } from './state/store.js';

(self as any).store = store;

const getQueryState = (): { [index: string]: string } =>
  Object.fromEntries(new URLSearchParams(location.search));

const applyQueryState = async () => {
  const queryState = getQueryState();

  console.log(queryState);

  if (queryState['id'] && queryState['version']) {
    await navigate(
      queryState['id'],
      queryState['version'],
      queryState['slug'] || null
    );
  }
};

export const navigate = async (
  id: string,
  version: string,
  slug: string | null
) => {
  console.log('Navigating:', id, version, slug);
  let state = store.getState();

  if (id && version && state.sphereViewer.noosphere && state.sphereViewer.key) {
    console.log('Open sphere...');
    await store.dispatch(
      openSphere({
        id,
        version,
        noosphere: state.sphereViewer.noosphere,
        key: state.sphereViewer.key,
      })
    );
  }

  state = store.getState();

  if (slug && state.sphereViewer.fs) {
    console.log('Open file...');
    await store.dispatch(
      openFile({
        fs: state.sphereViewer.fs,
        slug,
      })
    );
  }
};

let ipfsApi;

if (
  window.location.host.indexOf('localhost') == 0 ||
  window.location.host.indexOf('127.0.0.1') == 0
) {
  // For local dev, assume the default local gateway
  // configuration with permissive CORS
  ipfsApi = 'http://127.0.0.1:8080';
} else {
  ipfsApi = window.location.origin.toString();
}

await store.dispatch(
  connectToNoosphere({
    ipfsApi,
    key: 'anonymous',
  })
);

self.addEventListener(
  'click',
  (event: MouseEvent) => {
    const path = event.composedPath().reverse();

    for (const target of path) {
      if (target instanceof HTMLAnchorElement) {
        if (target.href == null) {
          continue;
        }

        const href = new URL(target.href, self.location.toString());

        if (href.origin === origin) {
          self.history.pushState(null, self.document.title, href);
          event.preventDefault();
          applyQueryState();
          break;
        }
      }
    }
  },
  true
);

self.addEventListener('popstate', () => applyQueryState());

applyQueryState();
