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
  sphereIndex: string[];
  slug: string | null;
  ipfsApi: string | null;
  key: string | null;
  noosphere: NoosphereContext | null;
  sphere: SphereContext | null;
  fs: SphereFs | null;
  fileContents: string | null;
  fileVersion: string | null;
  loading: boolean;
}

const initialState: SphereViewerState = {
  sphereViewerVersion: (self as any).SPHERE_VIEWER_VERSION || '',
  sphereViewerSha: (self as any).SPHERE_VIEWER_SHA || '',
  sphereId: null,
  sphereVersion: null,
  sphereIndex: [],
  slug: null,
  ipfsApi: null,
  key: null,
  noosphere: null,
  sphere: null,
  fs: null,
  fileContents: null,
  fileVersion: null,
  loading: true,
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
      state.loading = true;

      state.fileContents = null;
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
      state.fileContents = null;
      state.fileVersion = null;

      state.sphere = action.payload.sphere;
      state.fs = action.payload.fs;
    },

    fileOpened: (
      state,
      action: PayloadAction<{
        contents: string | null;
      }>
    ) => {
      state.fileContents = action.payload.contents;
      state.loading = false;
    },

    sphereIndexed: (state, action: PayloadAction<string[]>) => {
      state.sphereIndex = action.payload;

      if (!state.slug) {
        state.loading = false;
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
