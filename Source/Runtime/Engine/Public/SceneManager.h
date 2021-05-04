#pragma once

#include "Manager.h"
#include "Event.h"
#include "Scene.h"

class ENGINE_API SceneManager final : public Manager
{
public:
	[[nodiscard]] bool Update(float deltaTime) noexcept override;

	bool Load(Name name) noexcept;
	bool Save(Name name) const noexcept { return scenes[isFrontScene].Save(name); }
	bool Save() const noexcept { return scenes[isFrontScene].Save(); }

	void RegisterUpdate(const Delegate<void, float>& callback) { onUpdates[isFrontScene != isLoadScene] += callback; }
	void RegisterUpdate(Delegate<void, float>&& callback) { onUpdates[isFrontScene != isLoadScene] += std::move(callback); }

	Scene& GetScene() noexcept { return scenes[isFrontScene]; }
	const Scene& GetScene() const noexcept { return scenes[isFrontScene]; }

	void Exit() noexcept { isEnd = true; }

private:
	Event<void, float> onUpdates[2];
	Scene scenes[2];

	uint8 isFrontScene : 1;
	uint8 isLoadScene : 1;
	uint8 isSwapScene : 1;
	uint8 isEnd : 1;
};
