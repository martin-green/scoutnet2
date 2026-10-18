import { error, redirect } from '@sveltejs/kit';
import { api, ApiError, moduleCatalog } from '#lib/api/client.ts';
import { kindFromSlug } from '#lib/bodies.ts';
import type { LayoutLoad } from './$types';

// Enheten hämtas en gång för alla undersidor (översikt, medlemmar, väntelista, organisation).
// Modulkatalogen behövs för att avgöra om en undersida hör till en modul som är av.
export const load: LayoutLoad = async ({ params, url }) => {
	const kind = kindFromSlug(String(params.kind));
	try {
		const [body, catalog] = await Promise.all([api.body(`${kind}-${params.nr}`), moduleCatalog()]);
		return { body, catalog };
	} catch (e) {
		if (e instanceof ApiError && e.status === 401) redirect(307, `/login?return_to=${encodeURIComponent(url.pathname + url.search)}`);
		if (e instanceof ApiError && e.status === 404) error(404, 'Enheten finns inte, eller så har du inte behörighet att se den.');
		throw e;
	}
};
