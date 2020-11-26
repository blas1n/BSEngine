#pragma once

#include <string>
#include <vector>

namespace ArenaBoss
{
	class Scene;

	class SceneManager final
	{
	public:
		SceneManager(const std::string& inName);
		SceneManager(std::string&& inName);

		SceneManager(const SceneManager&) = delete;
		SceneManager(SceneManager&&) = delete;

		SceneManager& operator=(const SceneManager&) = delete;
		SceneManager& operator=(SceneManager&&) = delete;

		~SceneManager() = default;

		void ReserveScene(const std::string& inName);
		void ReserveScene(std::string&& inName);

		inline Scene& GetScene() noexcept { return *scene; }
		inline const Scene& GetScene() const noexcept { return *scene; }

		void Update();

	private:
		Scene* scene;
		std::string name;
		bool isReserved = false;
	};
}