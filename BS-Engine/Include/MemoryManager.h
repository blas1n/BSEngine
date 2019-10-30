#pragma once

#include "Core.h"
#include "IManager.h"
#include <vector>

class BS_API MemoryManager : public IManager
{
public:
	MemoryManager() noexcept;

	bool Init() noexcept override;
	void Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	void* Allocate(size_t n) noexcept;
	void Deallocate(void* ptr, size_t n) noexcept;

private:
	class HeapMemory* memory;
};