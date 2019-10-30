#include "MemoryManager.h"
#include "HeapMemory.h"

constexpr size_t MEMORY_SIZE = 30000;

MemoryManager::MemoryManager() noexcept
	: memory(nullptr) {}

bool MemoryManager::Init() noexcept
{
	memory = new HeapMemory{ };
	return memory->Init(MEMORY_SIZE);
}

void MemoryManager::Update(const float deltaTime) noexcept
{

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

void MemoryManager::Deallocate(void* ptr, const size_t n) noexcept
{
	memory->Free(ptr, n);
}