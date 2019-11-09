#include "ThreadManager.h"

bool ThreadManager::Init() noexcept
{
	auto threadNum = std::thread::hardware_concurrency();
	if (threadNum == 0) return false;

	threadNum = threadNum * 2 + 1;
	threads.reserve(threadNum);

	while (threadNum--)
		threads.emplace_back([this]() { this->ThreadWork(); });

	return true;
}

void ThreadManager::Release() noexcept
{
	isEnd = true;
	cv.notify_all();

	for (auto& t : threads)
		t.join();
}

void ThreadManager::ThreadWork()
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