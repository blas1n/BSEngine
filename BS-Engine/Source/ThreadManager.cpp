#include "ThreadManager.h"

bool ThreadManager::Init() noexcept
{
	auto threadNum = std::thread::hardware_concurrency();
	if (threadNum == 0) return false;
	
	threadNum = threadNum * 2 + 1;
	threads.reserve(threadNum);

	while (threadNum--)
		threads.emplace_back([this] { Work(); });

	return true;
}

void ThreadManager::Release() noexcept
{
	isEnd = true;
	cv.notify_all();

	for (auto& t : threads)
		t.join();
}

void ThreadManager::Work() noexcept
{
	while (true)
	{
		std::unique_lock<std::mutex> lock{ jobMutex };
		cv.wait(lock, [this] { return !tasks.empty() || isEnd; });
		if (isEnd) return;

		auto&& task = std::move(tasks.front());
		tasks.pop();
		lock.unlock();
		task();
	}
}