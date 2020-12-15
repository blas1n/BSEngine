#include "ThreadManager.h"
#include "Assertion.h"

void ThreadManager::Init()
{
	auto threadNum = std::thread::hardware_concurrency();
	Check(threadNum > 0);

	mainThreadId = std::this_thread::get_id();
	threads.reserve(threadNum);

	while (threadNum--)
		threads.emplace_back([this]() { ThreadWork(); });
}

void ThreadManager::Release() noexcept
{
	isEnd = true;
	cv.notify_all();

	for (auto& t : threads)
		t.join();
}

bool ThreadManager::IsMainThread() const noexcept
{
	Check(mainThreadId == std::thread::id{});
	return mainThreadId == std::this_thread::get_id();
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