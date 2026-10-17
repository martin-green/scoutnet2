import { api, ApiError, type Me } from '#lib/api/client.ts';

/** Inloggad användare, delas av hela appen (Svelte 5 runes i modul). */
class Session {
	me = $state<Me | null>(null);
	loading = $state(true);

	async load(): Promise<Me | null> {
		this.loading = true;
		try {
			this.me = await api.me();
		} catch (e) {
			if (!(e instanceof ApiError && e.status === 401)) throw e;
			this.me = null;
		} finally {
			this.loading = false;
		}
		return this.me;
	}

	async logout() {
		await api.logout();
		this.me = null;
	}

	can(bodyId: string, permission: string): boolean {
		return !!this.me?.contexts.some(
			(c) => c.body_id === bodyId && c.permissions.includes(permission as never)
		);
	}
}

export const session = new Session();
