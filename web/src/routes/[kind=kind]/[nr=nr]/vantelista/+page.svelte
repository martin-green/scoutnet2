<script lang="ts">
	import { api, type BodyChild } from '#lib/api/client.ts';
	import MemberGrid from '#lib/components/MemberGrid.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);
	const canManage = $derived(body.kind === 'group' && body.permissions.includes('members_manage'));

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

<header>
	<h1>Väntelista <span class="muted">– {body.name}</span></h1>
	<p class="muted">
		Sorterad på väntetid.
		{#if canManage}Markera de som ska få plats och välj <strong>Ta in i avdelning</strong>. Du ser vad som händer innan något ändras.{/if}
	</p>
</header>

{#key body.id}
	<MemberGrid bodyId={body.id} preset="waiting" {troops} {canManage} modules={body.modules} />
{/key}

<style>
	h1 .muted {
		font-weight: 400;
	}
</style>