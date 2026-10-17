import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

const api = 'http://127.0.0.1:8080';

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// SPA: allt renderas i webbläsaren. Rust-servern serverar build/ och faller tillbaka på 200.html.
			adapter: adapter({ fallback: '200.html' }),
			// Strikt CSP: inga tredjepartsskript, inline-skript bara med exakt hash.
			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'script-src': ['self'],
					'style-src': ['self', 'unsafe-inline'],
					'img-src': ['self', 'data:'],
					'connect-src': ['self'],
					'object-src': ['none'],
					'base-uri': ['none'],
					'form-action': ['self']
				}
			}
		})
	],
	server: {
		port: 5173,
		strictPort: true,
		proxy: {
			'/v1': api,
			'/auth': api,
			'/docs': api,
			'/openapi.json': api,
			'/healthz': api
		}
	}
});
