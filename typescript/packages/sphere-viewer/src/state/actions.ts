import { createAsyncThunk } from '@reduxjs/toolkit';
import { NoosphereContext, SphereFs } from '@subconsciousnetwork/orb';
import {
  fileOpened,
  ipfsConfigured,
  locationChanged,
  noosphereInitialized,
  sphereOpened,
  SphereViewerState,
} from './state.js';
import { RootState } from './store.js';

export const connectToNoosphere = createAsyncThunk(
  'sphereViewer/connectToNoosphere',
  async ({ ipfsApi, key }: { ipfsApi: string; key: string }, { dispatch }) => {
    dispatch(ipfsConfigured(ipfsApi));

    const noosphere = new NoosphereContext('sphere-viewer', undefined, ipfsApi);

    if (!(await noosphere.hasKey(key))) {
      await noosphere.createKey(key);
    }

    dispatch(noosphereInitialized({ noosphere, key }));
  }
);

export const openSphere = createAsyncThunk(
  'sphereViewer/openSphere',
  async (
    {
      noosphere,
      key,
      id,
      version,
    }: {
      noosphere: NoosphereContext;
      key: string;
      id: string;
      version: string;
    },
    { dispatch }
  ) => {
    dispatch(locationChanged({ id, version, slug: null }));

    await noosphere.joinSphere(id, key);

    let sphere = await noosphere.getSphereContext(id);
    let fs = await sphere.fsAt(version);

    dispatch(sphereOpened({ sphere, fs }));
  }
);

export const openFile = createAsyncThunk(
  'sphereViewer/openFile',
  async (
    { fs, slug }: { fs: SphereFs; slug: string },
    { dispatch, getState }
  ) => {
    let state = getState() as RootState;

    if (
      state.sphereViewer.sphereId == null ||
      state.sphereViewer.sphereVersion == null
    ) {
      return;
    }

    dispatch(
      locationChanged({
        id: state.sphereViewer.sphereId,
        version: state.sphereViewer.sphereVersion,
        slug,
      })
    );

    const file = (await fs.read(slug)) || null;
    const contents = (await file?.text()) || null;

    dispatch(fileOpened({ file, contents }));
  }
);
