// TypeScript code for JSONPlaceholder API simulation
// This simulates fetching data from https://jsonplaceholder.typicode.com/

interface Post {
  userId: number;
  id: number;
  title: string;
  body: string;
}

interface User {
  id: number;
  name: string;
  username: string;
  email: string;
  address: {
    street: string;
    suite: string;
    city: string;
    zipcode: string;
    geo: {
      lat: string;
      lng: string;
    };
  };
  phone: string;
  website: string;
  company: {
    name: string;
    catchPhrase: string;
    bs: string;
  };
}

interface PostComment {
  postId: number;
  id: number;
  name: string;
  email: string;
  body: string;
}

interface Album {
  userId: number;
  id: number;
  title: string;
}

interface Photo {
  albumId: number;
  id: number;
  title: string;
  url: string;
  thumbnailUrl: string;
}

interface Todo {
  userId: number;
  id: number;
  title: string;
  completed: boolean;
}

// Sample data that mimics JSONPlaceholder responses
const SAMPLE_POSTS: Post[] = [
  {
    userId: 1,
    id: 1,
    title: "sunt aut facere repellat provident occaecati excepturi optio reprehenderit",
    body: "quia et suscipit\nsuscipit recusandae consequuntur expedita et cum\nreprehenderit molestiae ut ut quas totam\nnostrum rerum est autem sunt rem eveniet architecto"
  },
  {
    userId: 1,
    id: 2,
    title: "qui est esse",
    body: "est rerum tempore vitae\nsequi sint nihil reprehenderit dolor beatae ea dolores neque\nfugiat blanditiis voluptate porro vel nihil molestiae ut reiciendis\nqui aperiam non debitis possimus qui neque nisi nulla"
  },
  {
    userId: 2,
    id: 3,
    title: "ea molestias quasi exercitationem repellat qui ipsa sit aut",
    body: "et iusto sed quo iure\nvoluptatem occaecati omnis eligendi aut ad\nvoluptatem doloribus vel accusantium quis pariatur\nmolestiae porro eius odio et labore et velit aut"
  }
];

const SAMPLE_USERS: User[] = [
  {
    id: 1,
    name: "Leanne Graham",
    username: "Bret",
    email: "Sincere@april.biz",
    address: {
      street: "Kulas Light",
      suite: "Apt. 556",
      city: "Gwenborough",
      zipcode: "92998-3874",
      geo: {
        lat: "-37.3159",
        lng: "81.1496"
      }
    },
    phone: "1-770-736-8031 x56442",
    website: "hildegard.org",
    company: {
      name: "Romaguera-Crona",
      catchPhrase: "Multi-layered client-server neural-net",
      bs: "harness real-time e-markets"
    }
  },
  {
    id: 2,
    name: "Ervin Howell",
    username: "Antonette",
    email: "Shanna@melissa.tv",
    address: {
      street: "Victor Plains",
      suite: "Suite 879",
      city: "Wisokyburgh",
      zipcode: "90566-7771",
      geo: {
        lat: "-43.9509",
        lng: "-34.4618"
      }
    },
    phone: "010-692-6593 x09125",
    website: "anastasia.net",
    company: {
      name: "Deckow-Crist",
      catchPhrase: "Proactive didactic contingency",
      bs: "synergize scalable supply-chains"
    }
  }
];

const SAMPLE_COMMENTS: PostComment[] = [
  {
    postId: 1,
    id: 1,
    name: "id labore ex et quam laborum",
    email: "Eliseo@gardner.biz",
    body: "laudantium enim quasi est quidem magnam voluptate ipsam eos\ntempora quo necessitatibus\ndolor quam autem quasi\nreiciendis et nam sapiente accusantium"
  },
  {
    postId: 1,
    id: 2,
    name: "quo vero reiciendis velit similique earum",
    email: "Jayne_Kuhic@sydney.com",
    body: "est natus enim nihil est dolore omnis voluptatem numquam\net omnis occaecati quod ullam at\nvoluptatem error expedita pariatur\nnihil sint nostrum voluptatem reiciendis et"
  }
];

const SAMPLE_ALBUMS: Album[] = [
  { userId: 1, id: 1, title: "quidem molestiae enim" },
  { userId: 1, id: 2, title: "sunt qui excepturi placeat culpa" },
  { userId: 2, id: 3, title: "omnis laborum odio" }
];

const SAMPLE_PHOTOS: Photo[] = [
  {
    albumId: 1,
    id: 1,
    title: "accusamus beatae ad facilis cum similique qui sunt",
    url: "https://via.placeholder.com/600/92c952",
    thumbnailUrl: "https://via.placeholder.com/150/92c952"
  },
  {
    albumId: 1,
    id: 2,
    title: "reprehenderit est deserunt velit ipsam",
    url: "https://via.placeholder.com/600/771796",
    thumbnailUrl: "https://via.placeholder.com/150/771796"
  }
];

const SAMPLE_TODOS: Todo[] = [
  { userId: 1, id: 1, title: "delectus aut autem", completed: false },
  { userId: 1, id: 2, title: "quis ut nam facilis et officia qui", completed: false },
  { userId: 1, id: 3, title: "fugiat veniam minus", completed: false },
  { userId: 2, id: 4, title: "et porro tempora", completed: true }
];

// Main function to simulate JSONPlaceholder API calls
function fetchJsonPlaceholderData(endpoint: string, id?: number): any {
  const startTime = Date.now();
  let data: any;
  let totalCount = 0;

  switch (endpoint) {
    case 'posts':
      if (id) {
        data = SAMPLE_POSTS.find(post => post.id === id) || null;
        if (!data) {
          return {
            success: false,
            error: `Post with id ${id} not found`,
            endpoint: `posts/${id}`,
            timestamp: new Date().toISOString(),
            processing_time_ms: Date.now() - startTime
          };
        }
      } else {
        data = SAMPLE_POSTS;
        totalCount = 100; // Simulate total posts available
      }
      break;

    case 'users':
      if (id) {
        data = SAMPLE_USERS.find(user => user.id === id) || null;
        if (!data) {
          return {
            success: false,
            error: `User with id ${id} not found`,
            endpoint: `users/${id}`,
            timestamp: new Date().toISOString(),
            processing_time_ms: Date.now() - startTime
          };
        }
      } else {
        data = SAMPLE_USERS;
        totalCount = 10; // Simulate total users available
      }
      break;

    case 'comments':
      if (id) {
        data = SAMPLE_COMMENTS.filter(comment => comment.postId === id);
      } else {
        data = SAMPLE_COMMENTS;
        totalCount = 500; // Simulate total comments available
      }
      break;

    case 'albums':
      if (id) {
        data = SAMPLE_ALBUMS.find(album => album.id === id) || null;
        if (!data) {
          return {
            success: false,
            error: `Album with id ${id} not found`,
            endpoint: `albums/${id}`,
            timestamp: new Date().toISOString(),
            processing_time_ms: Date.now() - startTime
          };
        }
      } else {
        data = SAMPLE_ALBUMS;
        totalCount = 100; // Simulate total albums available
      }
      break;

    case 'photos':
      if (id) {
        data = SAMPLE_PHOTOS.filter(photo => photo.albumId === id);
      } else {
        data = SAMPLE_PHOTOS;
        totalCount = 5000; // Simulate total photos available
      }
      break;

    case 'todos':
      if (id) {
        data = SAMPLE_TODOS.find(todo => todo.id === id) || null;
        if (!data) {
          return {
            success: false,
            error: `Todo with id ${id} not found`,
            endpoint: `todos/${id}`,
            timestamp: new Date().toISOString(),
            processing_time_ms: Date.now() - startTime
          };
        }
      } else {
        data = SAMPLE_TODOS;
        totalCount = 200; // Simulate total todos available
      }
      break;

    default:
      return {
        success: false,
        error: `Unknown endpoint: ${endpoint}`,
        available_endpoints: ['posts', 'users', 'comments', 'albums', 'photos', 'todos'],
        timestamp: new Date().toISOString(),
        processing_time_ms: Date.now() - startTime
      };
  }

  // Add metadata to simulate real API behavior
  const result = {
    success: true,
    data,
    metadata: {
      endpoint: id ? `${endpoint}/${id}` : endpoint,
      returned_count: Array.isArray(data) ? data.length : 1,
      total_available: totalCount || (Array.isArray(data) ? data.length : 1),
      api_source: "jsonplaceholder.typicode.com (simulated)",
      cached: Math.random() > 0.7 // Simulate some requests being cached
    },
    timestamp: new Date().toISOString(),
    processing_time_ms: Date.now() - startTime
  };

  return result;
}

// Helper function to get user's posts
function getUserPosts(userId: number): any {
  const startTime = Date.now();
  const userPosts = SAMPLE_POSTS.filter(post => post.userId === userId);
  const user = SAMPLE_USERS.find(u => u.id === userId);

  if (!user) {
    return {
      success: false,
      error: `User with id ${userId} not found`,
      timestamp: new Date().toISOString(),
      processing_time_ms: Date.now() - startTime
    };
  }

  return {
    success: true,
    data: {
      user: {
        id: user.id,
        name: user.name,
        username: user.username,
        email: user.email
      },
      posts: userPosts,
      stats: {
        total_posts: userPosts.length,
        avg_body_length: Math.round(userPosts.reduce((sum, post) => sum + post.body.length, 0) / userPosts.length) || 0
      }
    },
    metadata: {
      endpoint: `users/${userId}/posts`,
      processing_type: "aggregated_data",
      api_source: "jsonplaceholder.typicode.com (simulated)"
    },
    timestamp: new Date().toISOString(),
    processing_time_ms: Date.now() - startTime
  };
}

// Analytics function for JSONPlaceholder data
function analyzeJsonPlaceholderData(): any {
  const startTime = Date.now();

  const analytics = {
    posts: {
      total: SAMPLE_POSTS.length,
      avg_title_length: Math.round(SAMPLE_POSTS.reduce((sum, post) => sum + post.title.length, 0) / SAMPLE_POSTS.length),
      avg_body_length: Math.round(SAMPLE_POSTS.reduce((sum, post) => sum + post.body.length, 0) / SAMPLE_POSTS.length),
      posts_by_user: SAMPLE_POSTS.reduce((acc: any, post) => {
        acc[post.userId] = (acc[post.userId] || 0) + 1;
        return acc;
      }, {})
    },
    users: {
      total: SAMPLE_USERS.length,
      domains: SAMPLE_USERS.reduce((acc: any, user) => {
        const domain = user.email.split('@')[1];
        acc[domain] = (acc[domain] || 0) + 1;
        return acc;
      }, {}),
      cities: SAMPLE_USERS.map(user => user.address.city)
    },
    comments: {
      total: SAMPLE_COMMENTS.length,
      avg_body_length: Math.round(SAMPLE_COMMENTS.reduce((sum, comment) => sum + comment.body.length, 0) / SAMPLE_COMMENTS.length)
    },
    todos: {
      total: SAMPLE_TODOS.length,
      completed: SAMPLE_TODOS.filter(todo => todo.completed).length,
      completion_rate: Math.round((SAMPLE_TODOS.filter(todo => todo.completed).length / SAMPLE_TODOS.length) * 100)
    }
  };

  return {
    success: true,
    data: analytics,
    metadata: {
      endpoint: "analytics/overview",
      analysis_type: "comprehensive_stats",
      sample_size: {
        posts: SAMPLE_POSTS.length,
        users: SAMPLE_USERS.length,
        comments: SAMPLE_COMMENTS.length,
        todos: SAMPLE_TODOS.length
      }
    },
    timestamp: new Date().toISOString(),
    processing_time_ms: Date.now() - startTime
  };
}

// Export for V8
if (typeof globalThis !== 'undefined') {
  (globalThis as any).fetchJsonPlaceholderData = fetchJsonPlaceholderData;
  (globalThis as any).getUserPosts = getUserPosts;
  (globalThis as any).analyzeJsonPlaceholderData = analyzeJsonPlaceholderData;
}