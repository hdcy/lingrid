<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { languages, setLocale } from '../../i18n';
import { setThemeAccent, setThemeMode, themeAccent, themeAccents, themeMode, themeModes } from '../../shared/theme';
import { useNexusStore } from '../nexus/store';

const { t, te, locale } = useI18n();
const nexus = useNexusStore();
const apiKey = ref('');
const errorText = (code: string) => t(te(`errors.${code}`) ? `errors.${code}` : 'errors.unexpected');

// Show whether a key is already stored; the key itself never comes back from the backend.
void nexus.loadKeyStatus();

function saveKey() {
  void nexus.saveKey(apiKey.value).then(() => { apiKey.value = ''; });
}
</script>

<template>
  <section class="page">
    <header class="page-heading">
      <h1>{{ t('nav.settings') }}</h1>
      <p class="muted">{{ t('settings.description') }}</p>
    </header>
    <section class="panel settings-panel">
      <h2 class="section-title">{{ t('settings.appearance') }}</h2>
      <div class="setting-row">
        <div><label for="language">{{ t('language') }}</label><p id="language-hint" class="muted small">{{ t('settings.languageHint') }}</p></div>
        <select id="language" :value="locale" aria-describedby="language-hint" @change="setLocale(($event.target as HTMLSelectElement).value)">
          <option v-for="language in languages" :key="language.locale" :value="language.locale">{{ language.nativeName }}</option>
        </select>
      </div>
      <div class="setting-row">
        <div><span class="setting-label">{{ t('settings.themeMode') }}</span><p class="muted small">{{ t('settings.themeModeHint') }}</p></div>
        <div class="choices" role="group" :aria-label="t('settings.themeMode')">
          <button
            v-for="mode in themeModes"
            :key="mode"
            type="button"
            :aria-pressed="themeMode === mode"
            :class="{ active: themeMode === mode }"
            @click="setThemeMode(mode)"
          >{{ t(`settings.modes.${mode}`) }}</button>
        </div>
      </div>
      <div class="setting-row">
        <div><span class="setting-label">{{ t('settings.accent') }}</span><p class="muted small">{{ t('settings.accentHint') }}</p></div>
        <div class="swatches" role="group" :aria-label="t('settings.accent')">
          <button
            v-for="accent in themeAccents"
            :key="accent"
            type="button"
            class="swatch"
            :class="[`swatch-${accent}`, { active: themeAccent === accent }]"
            :aria-pressed="themeAccent === accent"
            :aria-label="t(`settings.accents.${accent}`)"
            :title="t(`settings.accents.${accent}`)"
            @click="setThemeAccent(accent)"
          ></button>
        </div>
      </div>
    </section>
    <section class="panel">
      <h2 class="section-title">{{ t('nexus.apiKeySection') }}</h2>
      <div class="setting-row">
        <div>
          <label for="nexus-key">{{ t('nexus.apiKeyLabel') }}</label>
          <p id="nexus-key-hint" class="muted small">{{ t('nexus.apiKeyHint') }}</p>
        </div>
        <input
          id="nexus-key"
          v-model="apiKey"
          type="password"
          dir="ltr"
          autocomplete="off"
          spellcheck="false"
          :disabled="nexus.busy"
          aria-describedby="nexus-key-hint"
        />
      </div>
      <div class="actions nexus-actions">
        <button :disabled="nexus.busy || !apiKey.trim()" @click="saveKey">{{ t('nexus.apiKeySave') }}</button>
        <span v-if="nexus.hasKey" class="muted small">{{ t('nexus.apiKeyStored') }}</span>
      </div>
      <div v-if="nexus.error" class="notice error" role="alert">{{ errorText(nexus.error.code) }}</div>
    </section>
    <section class="panel">
      <h2 class="section-title">{{ t('settings.about') }}</h2>
      <p>Lingrid</p>
      <p class="muted small">{{ t('shell.tagline') }}</p>
    </section>
  </section>
</template>

<style scoped>
.setting-label { font-weight: 550; }
.choices { display: flex; flex-wrap: wrap; gap: 6px; }
.choices button.active { background: var(--accent-soft); border-color: var(--accent-soft-border); color: var(--accent-text); font-weight: 650; }
.swatches { display: flex; flex-wrap: wrap; gap: 9px; }
.swatch { width: 30px; height: 30px; padding: 0; border-radius: 8px; border: 2px solid var(--border-strong); }
.swatch.active { border-color: var(--text); box-shadow: 0 0 0 2px var(--bg-panel) inset; }
.swatch-forest { background: #315d43; }
.swatch-ocean { background: #1f5d7a; }
.swatch-violet { background: #5b3f8f; }
.swatch-amber { background: #96601a; }
.nexus-actions { margin-top: 14px; align-items: center; }
#nexus-key { min-width: 240px; max-width: 100%; font-family: ui-monospace, monospace; font-size: 13px; }
</style>
