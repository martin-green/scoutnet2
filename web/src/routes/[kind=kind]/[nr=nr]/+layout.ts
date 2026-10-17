import { error, redirect } from '@sveltejs/kit';
import { api, ApiError } from '#lib/api/client.ts';
import { kindFromSlug } from '#lib/bodies.ts';
import type { LayoutLoad } from './$types';

// Enheten hämtas en gång för alla undersidor (översikt, medlemmar, väntelista, organisation).
export const load: LayoutLoad = async ({ params }) => {
	const kind = kindFromSlug(String(params.kind));
	try {
		return { body: await api.body(`${kind}-${params.nr}`) };
	} catch (e) {
		if (e instanceof ApiError && e.status === 401) redirect(307, '/login');
		if (e instanceof ApiError && e.status === 404) error(404, 'Enheten finns inte, eller så har du inte behörighet att se den.');
		throw e;
	}
};
