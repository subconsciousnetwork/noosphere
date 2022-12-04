import { configureStore } from '@reduxjs/toolkit';
import sphereViewerReducer from './state.js';

export const store = configureStore({
  reducer: {
    sphereViewer: sphereViewerReducer,
  },
});

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
