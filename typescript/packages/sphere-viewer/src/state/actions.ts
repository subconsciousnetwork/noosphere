import { createAsyncThunk } from '@reduxjs/toolkit';
import {
  NoosphereContext,
  SphereFile,
  SphereFs,
} from '@subconsciousnetwork/orb';
import {
  fileOpened,
  ipfsConfigured,
  locationChanged,
  noosphereInitialized,
  sphereIndexed,
  sphereOpened,
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
    { dispatch, getState }
  ) => {
    dispatch(locationChanged({ id, version, slug: null }));

    const state = getState() as RootState;

    if (
      state.sphereViewer.sphereId == id &&
      state.sphereViewer.sphereVersion == version &&
      state.sphereViewer.sphere
    ) {
      return;
    }

    await noosphere.joinSphere(id, key);

    const sphere = await noosphere.getSphereContext(id);
    const fs = await sphere.fsAt(version);

    dispatch(sphereOpened({ sphere, fs }));

    const sphereIndex = new Promise<string[]>(async (resolve, _) => {
      const sphereIndex: string[] = [];

      await fs.stream((slug: string, file: SphereFile) => {
        sphereIndex.push(slug);
        file.free();
      });

      resolve(sphereIndex);
    });

    dispatch(sphereIndexed(sphereIndex));
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

    const sphereId = state.sphereViewer.sphereId;
    const sphereVersion = state.sphereViewer.sphereVersion;

    dispatch(
      locationChanged({
        id: sphereId,
        version: sphereVersion,
        slug,
      })
    );

    const file = (await fs.read(slug)) || null;
    const version = file?.memoVersion() || null;
    const contents =
      file?.intoHtml((link: string) => {
        const url = new URL(window.location.toString());

        url.searchParams.set('id', sphereId);
        url.searchParams.set('version', sphereVersion);
        url.searchParams.set('slug', link.slice(1));

        return url.toString();
      }) || Promise.resolve(null);

    dispatch(
      fileOpened({
        contents,
        version,
      })
    );
  }
);
