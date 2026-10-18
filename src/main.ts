import { createApp } from "vue";
import App from "./App.vue";
import { createPinia } from 'pinia';
import { i18n } from './i18n';
import { router } from './app/router';
import './shared/theme';

createApp(App).use(createPinia()).use(i18n).use(router).mount("#app");
