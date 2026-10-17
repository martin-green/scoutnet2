<script lang="ts">
	import { api, fmtWhen, permissionLabels, type AttendanceSummary, type BodyChild } from '#lib/api/client.ts';
	import { bodyHref, kindPlural } from '#lib/bodies.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const body = $derived(data.body);

	let children = $state<BodyChild[]>([]);
	let attendance = $state<AttendanceSummary | null>(null);
	$effect(() => {
		const id = body.id;
		const canAttendance = body.permissions.includes('attendance_read') && ['troop', 'group'].includes(body.kind);
		let current = true;
		children = [];
		attendance = null;
		api.children(id)
			.then((c) => {
				if (current) children = c;
			})
			.catch(() => (children = []));
		if (canAttendance) {
			api.attendanceSummary(id)
				.then((s) => {
					if (current) attendance = s;
				})
				.catch(() => (attendance = null));
		}
		return () => (current = false);
	});
	const troopAttendance = $derived(body.kind === 'troop' ? attendance?.troops[0] : undefined);
	const missingAttendance = $derived(attendance?.troops.reduce((s, t) => s + t.missing_attendance, 0) ?? 0);

	const weekdays = ['', 'måndagar', 'tisdagar', 'onsdagar', 'torsdagar', 'fredagar', 'lördagar', 'söndagar'];
	const childHeading = $derived(
		children.length > 0 && children.every((c) => c.kind === children[0].kind)
			? (kindPlural[children[0].kind] ?? 'Underliggande enheter')
			: 'Underliggande enheter'
	);
	const hasWaitingList = $derived(['group', 'district', 'organisation'].includes(body.kind));
</script>

<header class="head">
	<div>
		<h1>{body.name}</h1>
		<p class="muted">
			{body.troop_type_label ?? body.kind_label} <span class="mono">{body.number}</span>{body.municipality ? ` · ${body.municipality}` : ''}
			{#if body.meeting}· Träffas {weekdays[body.meeting.weekday]} {body.meeting.start}{/if}
		</p>
	</div>
	<div class="actions">
		<a class="btn primary" href={bodyHref(body, 'medlemmar')}>👥 Medlemmar</a>
		{#if hasWaitingList && body.counts.waiting > 0}<a class="btn" href={bodyHref(body, 'vantelista')}>⏳ Väntelista</a>{/if}
		{#if children.length > 0}<a class="btn" href={bodyHref(body, 'organisation')}>🌳 Organisation</a>{/if}
	</div>
</header>

<section class="stats">
	<a class="card stat" href={bodyHref(body, 'medlemmar') + '?leaders=false'}>
		<span class="num">{body.counts.active - body.counts.leaders}</span><span class="muted">scouter</span>
	</a>
	<a class="card stat" href={bodyHref(body, 'medlemmar') + '?leaders=true'}>
		<span class="num">{body.counts.leaders}</span><span class="muted">ledare</span>
	</a>
	{#if hasWaitingList}
		<a class="card stat" href={bodyHref(body, 'vantelista')}>
			<span class="num">{body.counts.waiting}</span><span class="muted">i väntelistan</span>
		</a>
		<div class="card stat">
			<span class="num">{body.counts.pending}</span><span class="muted">väntar på godkännande</span>
		</div>
	{/if}
</section>

{#if troopAttendance}
	{@const t = troopAttendance}
	<section class="card attendance">
		<div>
			<h2>Närvaro {attendance?.term}</h2>
			<p class="muted">
				{t.held} av {t.planned} möten genomförda{t.attendance_pct != null ? ` · snittnärvaro ${Math.round(t.attendance_pct * 100)} %` : ''}
			</p>
		</div>
		<div class="att-actions">
			{#if t.last_meeting_id && t.last_meeting && t.last_meeting_recorded === 0}
				<a class="btn primary" href={`${bodyHref(body, 'narvaro')}/${t.last_meeting_id}`}>✅ Registrera närvaro – {fmtWhen(t.last_meeting)}</a>
			{:else if t.next_meeting_id && t.next_meeting}
				<a class="btn primary" href={`${bodyHref(body, 'narvaro')}/${t.next_meeting_id}`}>✅ Nästa möte – {fmtWhen(t.next_meeting)}</a>
			{/if}
			<a class="btn" href={bodyHref(body, 'narvaro')}>Närvarolista</a>
		</div>
	</section>
{:else if attendance && missingAttendance > 0}
	<section class="card attendance">
		<div>
			<h2>Att göra</h2>
			<p><span class="badge red">{missingAttendance} {missingAttendance === 1 ? 'möte saknar' : 'möten saknar'} närvaro</span> i kårens avdelningar {attendance.term}.</p>
		</div>
		<div class="att-actions"><a class="btn" href={bodyHref(body, 'narvaro')}>Visa närvaro</a></div>
	</section>
{/if}

{#if children.length > 0}
	<section>
		<h2>{childHeading}</h2>
		<div class="children">
			{#each children as c (c.id)}
				<a class="card child" href={bodyHref(c)}>
					<strong>{c.name}</strong>
					<span class="muted">{c.troop_type_label ?? c.kind_label} <span class="mono">{c.number}</span></span>
					<span class="counts">
						{#if c.troop_type === 'annan'}
							<span class="badge">{c.active} vuxna</span>
						{:else}
							<span class="badge blue">{c.active - c.leaders} scouter</span>
							<span class="badge accent">{c.leaders} ledare</span>
						{/if}
					</span>
				</a>
			{/each}
		</div>
	</section>
{/if}

<section class="card perms">
	<h2>Din behörighet här</h2>
	<p class="muted">Behörigheten kommer från dina roller och gäller den här enheten och allt under den.</p>
	<div class="chips">
		{#each body.permissions as p (p)}<span class="badge blue">{permissionLabels[p] ?? p}</span>{/each}
	</div>
</section>

<style>
	.head {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: var(--space-4);
		flex-wrap: wrap;
	}
	.actions {
		display: flex;
		gap: var(--space-2);
		flex-wrap: wrap;
	}
	.stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
		gap: var(--space-3);
		margin: var(--space-4) 0 var(--space-5);
	}
	.stat {
		display: flex;
		flex-direction: column;
		color: inherit;
		text-decoration: none;
	}
	a.stat:hover {
		border-color: var(--primary);
	}
	.num {
		font-size: 2rem;
		font-weight: 700;
		color: var(--primary-strong);
	}
	.children {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: var(--space-3);
		margin-bottom: var(--space-5);
	}
	.child {
		display: flex;
		flex-direction: column;
		gap: 4px;
		color: inherit;
		text-decoration: none;
	}
	.child:hover {
		border-color: var(--primary);
	}
	.counts {
		display: flex;
		gap: 4px;
		margin-top: 6px;
	}
	.perms .chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.attendance {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--space-3);
		flex-wrap: wrap;
		margin-bottom: var(--space-5);
	}
	.attendance h2,
	.attendance p {
		margin: 0 0 4px;
	}
	.att-actions {
		display: flex;
		gap: var(--space-2);
		flex-wrap: wrap;
	}
</style>
