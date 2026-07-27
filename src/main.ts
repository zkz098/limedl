import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { useNotificationStore } from "./stores/notification";
import "uno.css";
import "./styles.css";

const app = createApp(App);
app.use(createPinia());

app.config.errorHandler = (err, _instance, info) => {
  const message = err instanceof Error ? err.message : String(err);
  console.error("[Global Error]", err, info);
  useNotificationStore().notify(`Unexpected error: ${message}`, "error");
};

app.mount("#app");
