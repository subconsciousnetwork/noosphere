import './sphere-viewer.js';
import { openSphere, connectToNoosphere, openFile } from './state/actions.js';
import { store } from './state/store.js';

(self as any).store = store;

function getQueryState(): { [index: string]: string } {
  const query = self.location.search.substring(1);
  return query
    .split('&')
    .map((part) => {
      return part.split('=').map((subPart) => decodeURIComponent(subPart));
    })
    .reduce((map: { [index: string]: string }, entry) => {
      if (entry[0] && entry[1]) {
        map[entry[0]] = entry[1];
      }
      return map;
    }, {});
}

const queryState = getQueryState();

await store.dispatch(
  connectToNoosphere({
    ipfsApi: window.location.origin.toString(),
    key: 'anonyous',
  })
);

let state = store.getState();

if (
  queryState['id'] &&
  queryState['version'] &&
  state.sphereViewer.noosphere &&
  state.sphereViewer.key
) {
  await store.dispatch(
    openSphere({
      id: queryState['id'],
      version: queryState['version'],
      noosphere: state.sphereViewer.noosphere,
      key: state.sphereViewer.key,
    })
  );
}

state = store.getState();

if (queryState['slug'] && state.sphereViewer.fs) {
  await store.dispatch(
    openFile({
      fs: state.sphereViewer.fs,
      slug: queryState['slug'],
    })
  );
}
