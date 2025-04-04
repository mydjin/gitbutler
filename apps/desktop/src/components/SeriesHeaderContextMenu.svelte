<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { BranchController } from '$lib/branches/branchController';
	import { type CommitStatus } from '$lib/commits/commit';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { Project } from '$lib/project/project';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';

	interface Props {
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		headName: string;
		seriesCount: number;
		isTopBranch: boolean;
		hasForgeBranch: boolean;
		pr?: DetailedPullRequest;
		branchType: CommitStatus;
		description: string;
		stackId: string;
		toggleDescription: () => Promise<void>;
		onGenerateBranchName: () => void;
		onAddDependentSeries?: () => void;
		onOpenInBrowser?: () => void;
		onMenuToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	let {
		contextMenuEl = $bindable(),
		leftClickTrigger,
		rightClickTrigger,
		isTopBranch,
		seriesCount,
		hasForgeBranch,
		headName,
		pr,
		branchType,
		description,
		stackId,
		toggleDescription,
		onGenerateBranchName,
		onAddDependentSeries,
		onOpenInBrowser,
		onMenuToggle
	}: Props = $props();

	const project = getContext(Project);
	const aiService = getContext(AIService);
	const branchController = getContext(BranchController);
	const aiGenEnabled = projectAiGenEnabled(project.id);

	let deleteSeriesModal: Modal;
	let renameSeriesModal: Modal;
	let newHeadName: string = $state(headName);
	let isDeleting = $state(false);
	let aiConfigurationValid = $state(false);

	$effect(() => {
		setAIConfigurationValid();
	});

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	export function showSeriesRenameModal() {
		renameSeriesModal.show();
	}

	let isOpenedByMouse = $state(false);
</script>

<ContextMenu
	bind:this={contextMenuEl}
	{leftClickTrigger}
	{rightClickTrigger}
	ontoggle={(isOpen, isLeftClick) => {
		if (!isLeftClick) {
			isOpenedByMouse = true;
		} else {
			isOpenedByMouse = false;
		}

		onMenuToggle?.(isOpen, isLeftClick);
	}}
>
	{#if isOpenedByMouse && isTopBranch}
		<ContextMenuSection>
			<ContextMenuItem
				label="Add dependent branch"
				onclick={() => {
					onAddDependentSeries?.();
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}
	<ContextMenuSection>
		{#if isOpenedByMouse && hasForgeBranch}
			<ContextMenuItem
				label="Open in browser"
				onclick={() => {
					onOpenInBrowser?.();
					contextMenuEl?.close();
				}}
			/>
		{/if}
		<ContextMenuItem
			label="Copy branch name"
			onclick={() => {
				writeClipboard(headName);
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem
			label={`${!description ? 'Add' : 'Remove'} description`}
			onclick={async () => {
				await toggleDescription();
				contextMenuEl?.close();
			}}
		/>
		{#if $aiGenEnabled && aiConfigurationValid && !hasForgeBranch}
			<ContextMenuItem
				label="Generate branch name"
				onclick={() => {
					onGenerateBranchName();
					contextMenuEl?.close();
				}}
			/>
		{/if}
		{#if branchType !== 'Integrated'}
			<ContextMenuItem
				label="Rename"
				onclick={async () => {
					renameSeriesModal.show(stackId);
					contextMenuEl?.close();
				}}
			/>
		{/if}
		{#if seriesCount > 1}
			<ContextMenuItem
				label="Delete"
				onclick={() => {
					deleteSeriesModal.show(stackId);
					contextMenuEl?.close();
				}}
			/>
		{/if}
	</ContextMenuSection>
	{#if pr?.htmlUrl}
		<ContextMenuSection>
			<ContextMenuItem
				label="Open PR in browser"
				onclick={() => {
					openExternalUrl(pr.htmlUrl);
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Copy PR link"
				onclick={() => {
					writeClipboard(pr.htmlUrl);
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}
</ContextMenu>

<Modal
	width="small"
	title={hasForgeBranch ? 'Branch has already been pushed' : 'Rename branch'}
	type={hasForgeBranch ? 'warning' : 'info'}
	bind:this={renameSeriesModal}
	onSubmit={(close) => {
		if (newHeadName && newHeadName !== headName) {
			branchController.updateSeriesName(stackId, headName, newHeadName);
		}
		close();
	}}
>
	<Textbox placeholder="New name" id="newSeriesName" bind:value={newHeadName} autofocus />

	{#if hasForgeBranch}
		<div class="text-12 helper-text">
			Renaming a branch that has already been pushed will create a new branch at the remote. The old
			one will remain untouched but will be disassociated from this branch.
		</div>
	{/if}

	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit">Rename</Button>
	{/snippet}
</Modal>

<Modal
	width="small"
	title="Delete branch"
	bind:this={deleteSeriesModal}
	onSubmit={async (close) => {
		try {
			isDeleting = true;
			await branchController.removePatchSeries(stackId, headName);
			close();
		} finally {
			isDeleting = false;
		}
	}}
>
	{#snippet children()}
		Are you sure you want to delete <code class="code-string">{headName}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" loading={isDeleting}>Delete</Button>
	{/snippet}
</Modal>

<style>
	.helper-text {
		margin-top: 1rem;
		color: var(--clr-scale-ntrl-50);
		line-height: 1.5;
	}
</style>
