import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { useNotificationStore } from "./stores/notification";
import { toErrorMessage } from "./composables/downloadHelpers";
import "uno.css";
import "./styles.css";

const app = createApp(App);
app.use(createPinia());

app.config.errorHandler = (err, _instance, info) => {
  const message = toErrorMessage(err);
  console.error("[Global Error]", err, info);
  useNotificationStore().notify(`Unexpected error: ${message}`, "error");
};

app.mount("#app");
