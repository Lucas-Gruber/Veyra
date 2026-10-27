<script setup lang="ts">
import type { VaultItem } from '~/types/vault'
defineProps<{ item: VaultItem; large?: boolean }>()
</script>
<template><span class="item-icon" :class="{ large }" :style="{ '--item-color': item.color }" aria-hidden="true">{{ item.initials }}</span></template>
