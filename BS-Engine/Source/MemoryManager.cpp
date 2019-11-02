#include "MemoryManager.h"
#include "HeapMemory.h"
#include <algorithm>

MemoryManager::MemoryManager() noexcept
	: memory(nullptr) {}

bool MemoryManager::Init() noexcept
{
	memory = new HeapMemory{ };
	if (!memory->Init(MEMORY_SIZE))
		return false;
	return true;
}

void MemoryManager::Release() noexcept
{
	memory->Release();
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