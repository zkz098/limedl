import { createApp } from "vue";

import App from "./App.vue";
import { useNotification } from "./composables/useNotification";
import "uno.css";
import "./styles.css";

const app = createApp(App);

app.config.errorHandler = (err, _instance, info) => {
  const message = err instanceof Error ? err.message : String(err);
  console.error("[Global Error]", err, info);
  useNotification().notify(`Unexpected error: ${message}`, "error");
};

app.mount("#app");
