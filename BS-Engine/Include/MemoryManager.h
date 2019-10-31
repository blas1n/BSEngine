#pragma once

#include "IManager.h"
#include "IAllocatorBase.h"
#include "Array.h"

class BS_API MemoryManager final : public IManager
{
public:
	MemoryManager() noexcept;

	bool Init() noexcept override;
	void Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	void* Allocate(IAllocatorBase* alloc, size_t n) noexcept;
	void Deallocate(IAllocatorBase* alloc, void* ptr, size_t n) noexcept;

private:
	class HeapMemory* memory;
	Array<IAllocatorBase*>* singleFrameAllocators;
};