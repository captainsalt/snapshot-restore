git filter-branch --env-filter '
GIT_AUTHOR_NAME="captainsalt"
GIT_AUTHOR_EMAIL="dontbothertheking@gmail.com"
GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME"
GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
' -- --all