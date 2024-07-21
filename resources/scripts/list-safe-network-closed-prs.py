#!/usr/bin/env python

import os
import sys
from collections import defaultdict
from github import Github

def has_breaking_change(commits):
    for commit in commits:
        commit_message = commit.commit.message
        if '!' in commit_message.split('\n')[0] or 'BREAKING CHANGE' in commit_message:
            return True
    return False


def main(last_released_pr_number):
    token = os.getenv("GITHUB_PAT_SAFE_NETWORK_PR_LIST")
    if not token:
        raise Exception("The GITHUB_PAT_SAFE_NETWORK_PR_LIST environment variable must be set")

    g = Github(token)
    repo = g.get_repo("maidsafe/safe_network")

    last_released_pr = repo.get_pull(last_released_pr_number)
    if not last_released_pr:
        raise Exception(f"Could not retrieve PR #{last_released_pr_number}")
    last_release_date = last_released_pr.closed_at
    if not last_release_date:
        raise Exception(f"PR #{last_released_pr_number} has not been merged")

    print("Base comparison PR:")
    print(f"#{last_released_pr.number}: {last_released_pr.title} closed at {last_released_pr.closed_at}")
    print()

    pulls = repo.get_pulls(state="closed", sort="updated", direction="desc")
    filtered_pulls = []
    for pr in pulls:
        if not pr.closed_at:
            print(f"PR {pr.number} is not closed yet")
            continue
        print(f"Processing PR {pr.number}...")
        if pr.closed_at <= last_release_date:
            break
        if pr.merged_at:
            commits = pr.get_commits()
            breaking = has_breaking_change(commits)
            filtered_pulls.append({
                "number": pr.number,
                "title": pr.title,
                "author": pr.user.login,
                "closed_at": pr.closed_at,
                "breaking": breaking,
                "commits": commits
            })
    filtered_pulls.sort(key=lambda pr: pr["closed_at"])

    print("Flat list:")
    for pr in filtered_pulls:
        closed_date = pr["closed_at"].date()
        breaking_text = "[BREAKING]" if pr["breaking"] else ""
        print(f"{closed_date} #{pr['number']} -- {pr['title']} [@{pr['author']}] {breaking_text}")

    print()
    grouped_pulls = defaultdict(list)
    for pr in filtered_pulls:
        grouped_pulls[pr["author"]].append(pr)

    print("Grouped by author:")
    for author, prs in grouped_pulls.items():
        print(f"@{author}")
        for pr in prs:
            closed_date = pr["closed_at"].date()
            breaking_text = "[BREAKING]" if pr["breaking"] else ""
            print(f"  {closed_date} #{pr['number']} -- {pr['title']} {breaking_text}")
        print()

    print("Grouped by author with commits:")
    for author, prs in grouped_pulls.items():
        print(f"@{author}")
        for pr in prs:
            closed_date = pr["closed_at"].date()
            breaking_text = "[BREAKING]" if pr["breaking"] else ""
            print(f"  {closed_date} #{pr['number']} -- {pr['title']} {breaking_text}")
            for commit in pr["commits"]:
                print(f"    {commit.commit.message.split('\n')[0]}")
        print()

    print("Grouped by author markdown:")
    for author, prs in grouped_pulls.items():
        print(f"@{author}")
        for pr in prs:
            pr_number = pr["number"]
            closed_date = pr["closed_at"].date()
            breaking_text = "[BREAKING]" if pr["breaking"] else ""
            print(f"  {closed_date} [#{pr_number}](https://github.com/maidsafe/safe_network/pull/{pr_number}) -- {pr['title']} {breaking_text}")
        print()


if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python script.py <last_release_pr_number>")
        sys.exit(1)
    
    last_release_pr_number = int(sys.argv[1])
    main(last_release_pr_number)
