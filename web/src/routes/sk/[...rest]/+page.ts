import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

// Skojjt-v2 använder Scoutnets id i adresserna: /sk/{kår}, /sk/{kår}/t/{termin}/{avdelning} och
// /sk/{kår}/p/{medlemsnummer}. Samma nummer används i Scoutnet 2, så gamla bokmärken kan skickas vidare.
export const load: PageLoad = ({ params }) => {
	const [group, section, ...rest] = params.rest.split('/').filter(Boolean);
	if (!group || !/^\d+$/.test(group)) redirect(307, '/');
	const troop = section === 't' ? rest[1] : undefined;
	if (troop && /^\d+$/.test(troop)) redirect(308, `/avdelning/${troop}`);
	if (section === 'p' && rest[0] && /^\d+$/.test(rest[0])) redirect(308, `/kar/${group}/medlemmar?q=${rest[0]}`);
	redirect(308, `/kar/${group}`);
};
