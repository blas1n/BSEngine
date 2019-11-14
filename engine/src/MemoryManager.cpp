#include "MemoryManager.h"
#include "HeapMemory.h"
#include <algorithm>

MemoryManager::MemoryManager() noexcept
	: memory(nullptr) {}

bool MemoryManager::Init() noexcept
{
	memory = new HeapMemory{ MEMORY_SIZE };
	return true;
}

void MemoryManager::Release() noexcept
{
	delete memory;
}

void* MemoryManager::Allocate(const size_t n) noexcept
{
	return memory->Malloc(n);
}

void MemoryManager::Deallocate(void* const ptr, const size_t n) noexcept
{
	memory->Free(ptr, n);
}