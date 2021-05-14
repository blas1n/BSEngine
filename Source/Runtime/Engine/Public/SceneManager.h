#pragma once

#include "Manager.h"
#include <future>
#include <shared_mutex>
#include "Accessor.h"
#include "Event.h"
#include "Scene.h"

class ENGINE_API SceneManager final : public Manager, private Accessor<class ThreadManager>
{
public:
	[[nodiscard]] bool Update(float deltaTime) noexcept override;

	std::future<bool> Load(Name name) noexcept;
	bool Save(Name name) const noexcept { return scenes[isFrontScene].Save(name); }
	bool Save() const noexcept { return scenes[isFrontScene].Save(); }

	void RegisterUpdate(const Delegate<void(float)>& callback);
	void RegisterUpdate(Delegate<void(float)>&& callback);

	[[nodiscard]] Scene& GetScene() noexcept { return scenes[isFrontScene]; }
	[[nodiscard]] const Scene& GetScene() const noexcept { return scenes[isFrontScene]; }

	void Exit() noexcept { isEnd = true; }

private:
	bool LoadImpl(Name name) noexcept;

private:
	Event<void(float)> onUpdates[2];
	Scene scenes[2];

	std::shared_mutex mutex;

	uint8 isFrontScene : 1;
	uint8 isLoadScene : 1;
	uint8 isSwapScene : 1;
	uint8 isEnd : 1;
};
