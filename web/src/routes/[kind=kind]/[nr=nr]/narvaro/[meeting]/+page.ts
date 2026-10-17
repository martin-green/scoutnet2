import { error, redirect } from '@sveltejs/kit';
import { api, ApiError } from '#lib/api/client.ts';
import { bodyHref } from '#lib/bodies.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params, parent }) => {
	const { body } = await parent();
	let detail;
	try {
		detail = await api.meeting(params.meeting);
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) error(404, 'Mötet finns inte, eller så har du inte behörighet att se det.');
		throw e;
	}
	// Mötet ska visas under sin egen avdelning, så att brödsmulor och meny stämmer.
	if (detail.meeting.troop.id !== body.id) redirect(307, `${bodyHref(detail.meeting.troop, 'narvaro')}/${detail.meeting.id}`);
	return { detail, crumb: detail.meeting.name };
};
