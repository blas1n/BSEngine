#include "ThreadManager.h"

bool ThreadManager::Init() noexcept
{
	const auto threadNum = std::thread::hardware_concurrency();
	threads.reserve(threadNum);

	for (auto i = 0u; i < threadNum; ++i)
		threads.emplace_back([this] { Work(); });

	return true;
}

void ThreadManager::Update(const float deltaTime) noexcept
{

}

void ThreadManager::Release() noexcept
{
	isEnd = true;
	cv.notify_all();

	for (auto& t : threads)
		t.join();
}