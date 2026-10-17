<script lang="ts">
	import { api, ApiError, feeStatusLabels, fmtDate, fmtSek, policeCheckLabels, type PersonDetail } from '#lib/api/client.ts';
	import { bodyHref } from '#lib/bodies.ts';
	import PersonPanel from '#lib/components/PersonPanel.svelte';
	import { session } from '#lib/session.svelte.ts';

	let person = $state<PersonDetail | null>(null);
	let missing = $state(false);
	let open = $state(false);

	$effect(() => {
		const me = session.me;
		if (!me) return;
		api.person(me.person_id)
			.then((p) => (person = p))
			.catch((e) => {
				if (e instanceof ApiError && e.status === 404) missing = true;
			});
	});
	const address = $derived(person?.addresses?.find((a) => a.is_primary) ?? person?.addresses?.[0]);
	const currentFee = $derived(person?.fees?.[0]);
</script>

<h1>Min profil</h1>

{#if session.me}
	<div class="grid">
		<section class="card">
			<h2>{session.me.name}</h2>
			<p class="muted">Medlemsnummer <span class="mono">{session.me.member_no}</span></p>
			{#if person}
				<dl>
					<dt>E-post</dt>
					<dd>{person.contact?.email ?? '–'}</dd>
					<dt>Mobil</dt>
					<dd>{person.contact?.mobile_phone ?? '–'}</dd>
					{#if address}
						<dt>Adress</dt>
						<dd>{address.line1}<br />{address.postcode} {address.city}</dd>
					{/if}
					<dt>Född</dt>
					<dd>{fmtDate(person.birth_date)}</dd>
					<dt>Kår</dt>
					<dd>{person.membership.group.name}</dd>
					{#if person.troop}<dt>Avdelning</dt><dd>{person.troop.name}</dd>{/if}
					{#if person.patrol}<dt>Patrull</dt><dd>{person.patrol.name}</dd>{/if}
					<dt>Närvaro {person.attendance.term}</dt>
					<dd>{person.attendance.present} av {person.attendance.total}</dd>
					{#if currentFee}
						{@const s = feeStatusLabels[currentFee.status]}
						<dt>Avgift {currentFee.term}</dt>
						<dd>{fmtSek(currentFee.amount_ore)} <span class="badge {s.tone}">{s.label}</span></dd>
					{/if}
					{#if person.police_check?.required}
						{@const pc = policeCheckLabels[person.police_check.status]}
						<dt>Registerutdrag</dt>
						<dd><span class="badge {pc.tone}">{pc.label}</span>{#if person.police_check.valid_until} <span class="muted">till {fmtDate(person.police_check.valid_until)}</span>{/if}</dd>
					{/if}
				</dl>
				<button class="btn" onclick={() => (open = true)}>Visa alla mina uppgifter</button>
			{:else if missing}
				<p class="muted">Du är registrerad som vårdnadshavare. Vyn "Mina scouter", med anmälan av förhinder och fakturor, kommer i vecka 2.</p>
			{/if}
		</section>

		<section class="card">
			<h2>Roller</h2>
			{#if session.me.roles.length === 0}
				<p class="muted">Du har inga funktionärsroller.</p>
			{:else}
				<ul>
					{#each session.me.roles as r (r.body_id + r.role_key)}
						<li>{r.role_label} – <a href={bodyHref({ kind: r.body_kind, number: r.body_number })}>{r.body_name}</a></li>
					{/each}
				</ul>
			{/if}
			<h2>Integritet</h2>
			<p class="muted">
				Kommer vecka 5: se vem som har tittat på dina uppgifter (revisionsloggen), ladda ner dina uppgifter (registerutdrag) och hantera samtycken.
			</p>
		</section>
	</div>
	{#if open && person}
		<PersonPanel id={person.id} onclose={() => (open = false)} />
	{/if}
{/if}

<style>
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
		gap: var(--space-4);
	}
	dl {
		display: grid;
		grid-template-columns: 140px 1fr;
		gap: 6px var(--space-3);
	}
	dt {
		color: var(--muted);
	}
	dd {
		margin: 0;
	}
</style>
