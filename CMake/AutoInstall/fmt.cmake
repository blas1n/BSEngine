cmake_minimum_required (VERSION 3.10)
project (${NAME}-DOWNLOAD)
include (ExternalProject)

message ("Dir: ${SRC_DIR}, ${BUILD_DIR}")

ExternalProject_Add (${NAME}-DOWNLOAD
	GIT_REPOSITORY		https://github.com/fmtlib/fmt.git
	GIT_TAG				7.1.3
	GIT_SHALLOW			TRUE
	GIT_PROGRESS		TRUE
	SOURCE_DIR			"${SRC_DIR}"
	BINARY_DIR			"${BUILD_DIR}"
	CONFIGURE_COMMAND	""
	BUILD_COMMAND		""
	INSTALL_COMMAND		""
	TEST_COMMAND		""
)
