#include "System.h"
#include "MemoryManager.h"
#include "ThreadManager.h"
#include "ManagerAllocator.h"
#include "ThreadManagerAccesser.h"

// Temp variable, (Use config file)
constexpr BE::SizeType MANAGER_SIZE = sizeof(BE::ThreadManager);
constexpr BE::SizeType COMPONENT_SIZE = 1000;
constexpr BE::SizeType RESOURCE_SIZE = 1000;
constexpr BE::SizeType ONE_FRAME_SIZE = 1000;
constexpr BE::SizeType HEAP_SIZE = 10000;

namespace BE
{
	static MemoryManager memoryManager;

	void System::Initialize() noexcept {
		memoryManager.Init({ MANAGER_SIZE, COMPONENT_SIZE, RESOURCE_SIZE, ONE_FRAME_SIZE, HEAP_SIZE });
		MemoryManagerAccesser::Set(&memoryManager);
		
		ManagerAllocator allocator;

		// Allocate memory and call constructor each manager.
#pragma push_macro("new")
#undef new
		threadManager = new(allocator.Allocate<ThreadManager>())ThreadManager();
#pragma pop_macro("new")

		// Init and set accesser each manager.
		threadManager->Init();
		ThreadManagerAccesser::Set(threadManager);
	}

	Int32 System::RunLoop() noexcept {
		return 0;
	}

	void System::Release() noexcept {
		// Release each manager.
		threadManager->Release();

		memoryManager.Release();
	}
}