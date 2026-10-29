// SPDX-License-Identifier: AGPL-3.0-or-later
import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';

// The offline shell (src/service-worker.ts), built next to the app; not in development, where it would cache stale
// modules.
if (import.meta.env.PROD && 'serviceWorker' in navigator) {
	void navigator.serviceWorker.register('/service-worker.js');
}

export default mount(App, { target: document.getElementById('app')! });
