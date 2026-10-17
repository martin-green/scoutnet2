import { error, redirect } from '@sveltejs/kit';
import { api } from '#lib/api/client.ts';
import { bodyHref } from '#lib/bodies.ts';
import type { PageLoad } from './$types';

// Äldre prototyplänkar med UUID (/enhet/<uuid>/medlemmar) skickas vidare till läsbara adresser.
export const load: PageLoad = async ({ params, url }) => {
	const body = await api.body(params.id).catch(() => null);
	if (!body) error(404, 'Enheten finns inte, eller så har du inte behörighet att se den.');
	redirect(308, bodyHref(body, params.rest) + url.search);
};
