import { createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';
import {
  NoosphereContext,
  SphereFs,
  SphereFile,
  SphereContext,
} from '@subconsciousnetwork/orb';

export interface SphereViewerState {
  sphereViewerVersion: string;
  sphereViewerSha: string;
  sphereId: string | null;
  sphereVersion: string | null;
  sphereIndex: Promise<string[]>;
  slug: string | null;
  ipfsApi: string | null;
  key: string | null;
  noosphere: NoosphereContext | null;
  sphere: SphereContext | null;
  fs: SphereFs | null;
  fileContents: Promise<string | null>;
  fileVersion: string | null;
  loading: Promise<void>;
}

const initialState: SphereViewerState = {
  sphereViewerVersion: (self as any).SPHERE_VIEWER_VERSION || '',
  sphereViewerSha: (self as any).SPHERE_VIEWER_SHA || '',
  sphereId: null,
  sphereVersion: null,
  sphereIndex: Promise.resolve([]),
  slug: null,
  ipfsApi: null,
  key: null,
  noosphere: null,
  sphere: null,
  fs: null,
  fileContents: Promise.resolve(null),
  fileVersion: null,
  loading: new Promise(() => {}),
};

export const sphereViewerSlice = createSlice({
  name: 'sphereViewer',
  initialState,
  reducers: {
    ipfsConfigured: (state, action: PayloadAction<string>) => {
      state.ipfsApi = action.payload;
    },

    noosphereInitialized: (
      state,
      action: PayloadAction<{
        noosphere: NoosphereContext;
        key: string;
      }>
    ) => {
      if (state.noosphere) {
        state.noosphere.free();
      }
      state.noosphere = action.payload.noosphere;
      state.key = action.payload.key;
    },

    locationChanged: (
      state,
      action: PayloadAction<{
        id: string;
        version: string;
        slug: string | null;
      }>
    ) => {
      state.sphereId = action.payload.id;
      state.sphereVersion = action.payload.version;
      state.slug = action.payload.slug;
      state.loading = Promise.resolve();

      state.fileContents = Promise.resolve(null);
      state.fileVersion = null;
    },

    sphereOpened: (
      state,
      action: PayloadAction<{ sphere: SphereContext; fs: SphereFs }>
    ) => {
      if (state.sphere) {
        state.sphere.free();
      }

      if (state.fs) {
        state.fs.free();
      }

      state.slug = null;
      state.fileContents = Promise.resolve(null);
      state.fileVersion = null;

      state.sphere = action.payload.sphere;
      state.fs = action.payload.fs;
    },

    fileOpened: (
      state,
      action: PayloadAction<{
        contents: Promise<string | null>;
        version: string | null;
      }>
    ) => {
      state.fileContents = action.payload.contents;
      state.fileVersion = action.payload.version;
      state.loading = state.fileContents.then(() => {});
    },

    sphereIndexed: (state, action: PayloadAction<Promise<string[]>>) => {
      state.sphereIndex = action.payload;

      if (!state.slug) {
        state.loading = state.sphereIndex.then(() => {});
      }
    },
  },
});

// Action creators are generated for each case reducer function
export const {
  ipfsConfigured,
  noosphereInitialized,
  locationChanged,
  sphereOpened,
  sphereIndexed,
  fileOpened,
} = sphereViewerSlice.actions;

export default sphereViewerSlice.reducer;
