import { createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';
import {
  NoosphereContext,
  SphereFs,
  SphereFile,
  SphereContext,
} from '@subconsciousnetwork/orb';

export interface SphereViewerState {
  sphereId: string | null;
  sphereVersion: string | null;
  slug: string | null;
  ipfsApi: string | null;
  key: string | null;
  noosphere: NoosphereContext | null;
  sphere: SphereContext | null;
  fs: SphereFs | null;
  file: SphereFile | null;
  fileContents: string | null;
}

const initialState: SphereViewerState = {
  sphereId: null,
  sphereVersion: null,
  slug: null,
  ipfsApi: null,
  key: null,
  noosphere: null,
  sphere: null,
  fs: null,
  file: null,
  fileContents: null,
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

      state.sphere = action.payload.sphere;
      state.fs = action.payload.fs;
    },

    fileOpened: (
      state,
      action: PayloadAction<{
        file: SphereFile | null;
        contents: string | null;
      }>
    ) => {
      if (state.file) {
        state.file.free();
      }

      state.file = action.payload.file;
      state.fileContents = action.payload.contents;
    },

    fileContentsRead: (state, action: PayloadAction<string>) => {
      state.fileContents = action.payload;
    },
  },
});

// Action creators are generated for each case reducer function
export const {
  ipfsConfigured,
  noosphereInitialized,
  locationChanged,
  sphereOpened,
  fileOpened,
  fileContentsRead,
} = sphereViewerSlice.actions;

export default sphereViewerSlice.reducer;
