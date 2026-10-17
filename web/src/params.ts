import { defineParams } from '@sveltejs/kit/params';
import { isKindSlug } from '#lib/bodies.ts';

// Matchare för läsbara enhetsadresser: /[kind=kind]/[nr=nr], t.ex. /kar/1001.
export const params = defineParams({
	kind: (param) => (isKindSlug(param) ? param : undefined),
	// Bara kanoniska nummer, så att varje enhet har exakt en adress.
	nr: (param) => (/^[1-9]\d{0,8}$/.test(param) ? Number(param) : undefined)
});
