#include "ThreadManager.h"
#include "Assertion.h"

bool ThreadManager::Init() noexcept
{
	uint32 threadNum = std::thread::hardware_concurrency();
	if (!Ensure(threadNum > 0))
		return false;

	mainThreadId = std::this_thread::get_id();
	threads.reserve(threadNum);

	while (threadNum--)
		threads.emplace_back([this]() { ThreadWork(); });

	return true;
}

void ThreadManager::Release() noexcept
{
	isEnd = true;
	cv.notify_all();

	for (auto& t : threads)
		t.join();
}

void ThreadManager::ThreadWork() noexcept
{
	while (true)
	{
		std::unique_lock<std::mutex> lock{ taskMutex };
		cv.wait(lock, [this]() { return !tasks.empty() || isEnd; });
		if (isEnd && tasks.empty()) return;

		auto task = std::move(tasks.front());
		tasks.pop();
		lock.unlock();
		task();
	}
}
