<script lang="ts">
	import MemberGrid from '#lib/components/MemberGrid.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);
</script>

<header>
	<h1>Väntelista <span class="muted">– {body.name}</span></h1>
	<p class="muted">Sorterad på väntetid. Nästa steg (vecka 5): erbjud plats direkt härifrån, med svarsfrist för vårdnadshavaren.</p>
</header>

{#key body.id}
	<MemberGrid bodyId={body.id} preset="waiting" />
{/key}

<style>
	h1 .muted {
		font-weight: 400;
	}
</style>
