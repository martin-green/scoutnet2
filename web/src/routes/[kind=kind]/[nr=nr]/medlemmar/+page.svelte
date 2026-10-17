<script lang="ts">
	import { api, type BodyChild } from '#lib/api/client.ts';
	import MemberGrid from '#lib/components/MemberGrid.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);

	let troops = $state<BodyChild[]>([]);
	$effect(() => {
		const id = body.id;
		let current = true;
		api.children(id)
			.then((c) => {
				if (current) troops = c.filter((x) => x.kind === 'troop');
			})
			.catch(() => (troops = []));
		return () => (current = false);
	});
</script>

<header class="head">
	<h1>Medlemmar <span class="muted">– {body.name}</span></h1>
</header>

{#key body.id}
	<MemberGrid bodyId={body.id} {troops} canManage={body.kind === 'group' && body.permissions.includes('members_manage')} />
{/key}

<style>
	.head h1 .muted {
		font-weight: 400;
	}
</style>
