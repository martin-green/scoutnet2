<script lang="ts">
	import OrgTree from '#lib/components/OrgTree.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<header>
	<h1>Organisation <span class="muted">– {data.body.name}</span></h1>
	<p class="muted">
		Hela strukturen under {data.body.name}: fäll ut trädet, eller sök och lista enheter för att jämföra många på en gång.
	</p>
</header>

{#key data.body.id}
	<OrgTree root={data.body} />
{/key}

<style>
	h1 .muted {
		font-weight: 400;
	}
</style>
