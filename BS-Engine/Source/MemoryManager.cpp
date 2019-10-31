#include "MemoryManager.h"
#include "HeapMemory.h"
#include <algorithm>

constexpr size_t MEMORY_SIZE = 30000;
constexpr size_t MAX_SINGLE_FRAME_ALLOCATOR = 100;

MemoryManager::MemoryManager() noexcept
	: memory(nullptr),
	singleFrameAllocators(nullptr) {}

bool MemoryManager::Init() noexcept
{
	memory = new HeapMemory{ };
	if (!memory->Init(MEMORY_SIZE))
		return false;

	constexpr auto singleFrameSize = MAX_SINGLE_FRAME_ALLOCATOR * sizeof(IAllocatorBase*);

	PoolAllocator<IAllocatorBase*> alloc{ MAX_SINGLE_FRAME_ALLOCATOR };

	singleFrameAllocators =
		static_cast<Array<IAllocatorBase*>*>(Allocate(&alloc, sizeof(Array<IAllocatorBase*>)));
	
#pragma push_macro("new")
#undef new
	singleFrameAllocators = new(singleFrameAllocators)Array<IAllocatorBase*>{ alloc };
#pragma pop_macro("new")

	return singleFrameAllocators != nullptr;
}

void MemoryManager::Update(const float deltaTime) noexcept
{
	for (const auto alloc : *singleFrameAllocators)
		alloc->Clear();
}

void MemoryManager::Release() noexcept
{
	memory->Release();
	delete memory;
}

void* MemoryManager::Allocate(IAllocatorBase* const alloc, const size_t n) noexcept
{
	if (alloc->IsSingleFrame())
		singleFrameAllocators->emplace_back(alloc);

	return memory->Malloc(n);
}

void MemoryManager::Deallocate(IAllocatorBase* const alloc, void* const ptr, const size_t n) noexcept
{
	if (alloc->IsSingleFrame())
	{
		const auto iter = std::find(
			singleFrameAllocators->cbegin(), singleFrameAllocators->cend(), alloc);

		check(iter != singleFrameAllocators->cend());
		singleFrameAllocators->erase(iter);
	}

	memory->Free(ptr, n);
}