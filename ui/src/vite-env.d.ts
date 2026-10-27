/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Base URL of the weight-check backend; defaults to http://127.0.0.1:8787. */
  readonly VITE_API_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
