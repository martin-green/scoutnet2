<script lang="ts">
	import { goto } from '$app/navigation';
	import { bodyHref } from '#lib/bodies.ts';
	import { session } from '#lib/session.svelte.ts';

	// Startsidan skickar vidare till första arbetskontexten, eller till Min profil.
	$effect(() => {
		const me = session.me;
		if (!me) return;
		const first = me.contexts[0];
		goto(first ? bodyHref({ kind: first.body_kind, number: first.body_number }) : '/mig', { replace: true });
	});
</script>

<p class="muted">Laddar…</p>
