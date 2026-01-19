import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listDockerContainers, startDockerContainer, stopDockerContainer, restartDockerContainer, getDockerLogs } from '../api/docker';

// Mock the client module
vi.mock('../api/client', () => ({
  callApi: vi.fn(),
}));

import { callApi } from '../api/client';

describe('Docker API Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listDockerContainers', () => {
    it('should fetch containers and parse them correctly', async () => {
      const mockContainers = [
        {
          name: 'nginx',
          status: 'running',
          state: 'running',
          health_status: 'healthy',
          uptime_seconds: 3600,
          image: 'nginx:latest',
          ports: ['80:80', '443:443'],
          labels: { app: 'web' },
          created_at: '2024-01-01T00:00:00Z',
          restart_count: 0,
        },
        {
          name: 'redis',
          status: 'running', 
          state: 'running',
          health_status: null,
          uptime_seconds: 7200,
          image: 'redis:alpine',
          ports: ['6379:6379'],
          labels: {},
          created_at: '2024-01-02T00:00:00Z',
          restart_count: 2,
        },
      ];

      vi.mocked(callApi).mockResolvedValueOnce({
        status: 200,
        data: { 
          containers: mockContainers,
          timestamp: '2024-01-15T12:00:00Z'
        }
      });

      const result = await listDockerContainers();

      expect(callApi).toHaveBeenCalledWith({
        path: '/api/docker',
        method: 'GET',
      });

      expect(result.containers).toHaveLength(2);
      expect(result.containers[0].name).toBe('nginx');
      expect(result.containers[0].health_status).toBe('healthy');
      expect(result.containers[1].name).toBe('redis');
      expect(result.containers[1].health_status).toBe(null);
      expect(result.timestamp).toBe('2024-01-15T12:00:00Z');
    });

    it('should throw error when Docker service is unavailable', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 503,
        data: null
      });

      await expect(listDockerContainers()).rejects.toThrow('Docker service is not available');
    });

    it('should throw error for invalid response shape', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 200,
        data: { containers: 'not-an-array' } // Invalid
      });

      await expect(listDockerContainers()).rejects.toThrow('Invalid Docker list response shape');
    });
  });

  describe('startDockerContainer', () => {
    it('should start container and return success', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 200,
        data: { success: true, message: 'Container started' }
      });

      const result = await startDockerContainer('nginx');

      expect(callApi).toHaveBeenCalledWith({
        path: '/api/docker/nginx/start',
        method: 'POST',
      });

      expect(result.success).toBe(true);
      expect(result.message).toBe('Container started');
    });

    it('should throw error when container not found', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 404,
        data: null
      });

      await expect(startDockerContainer('nonexistent')).rejects.toThrow('Container "nonexistent" not found');
    });

    it('should throw error when Docker service unavailable', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 503,
        data: null
      });

      await expect(startDockerContainer('nginx')).rejects.toThrow('Docker service is not available');
    });
  });

  describe('stopDockerContainer', () => {
    it('should stop container with timeout', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 200,
        data: { success: true, message: 'Container stopped', stopped: true }
      });

      const result = await stopDockerContainer('nginx', 30);

      expect(callApi).toHaveBeenCalledWith({
        path: '/api/docker/nginx/stop',
        method: 'POST',
        body: { timeout_seconds: 30 },
      });

      expect(result.success).toBe(true);
      expect(result.stopped).toBe(true);
    });

    it('should handle already stopped container', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 200,
        data: { success: true, message: 'Container was not running', stopped: false }
      });

      const result = await stopDockerContainer('nginx', 10);

      expect(result.stopped).toBe(false);
    });
  });

  describe('restartDockerContainer', () => {
    it('should restart container with timeout', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 200,
        data: { success: true, message: 'Container restarted' }
      });

      const result = await restartDockerContainer('nginx', 10);

      expect(callApi).toHaveBeenCalledWith({
        path: '/api/docker/nginx/restart',
        method: 'POST',
        body: { timeout_seconds: 10 },
      });

      expect(result.success).toBe(true);
    });

    it('should throw error when container not found', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 404,
        data: null
      });

      await expect(restartDockerContainer('nonexistent', 10)).rejects.toThrow('Container "nonexistent" not found');
    });
  });

  describe('getDockerLogs', () => {
    it('should fetch logs with tail parameter', async () => {
      const mockLogs = '2024-01-15T12:00:00Z log line 1\n2024-01-15T12:00:01Z log line 2';

      vi.mocked(callApi).mockResolvedValueOnce({
        status: 200,
        data: mockLogs
      });

      const result = await getDockerLogs('nginx', { tail: 100 });

      expect(callApi).toHaveBeenCalled();
      
      // Check that the path contains the expected query params
      const calledPath = (vi.mocked(callApi).mock.calls[0][0] as { path: string }).path;
      expect(calledPath).toContain('/api/docker/nginx/logs?');
      expect(calledPath).toContain('tail=100');
      // timestamps is only added when explicitly provided
      expect(calledPath).not.toContain('timestamps');

      expect(result).toBe(mockLogs);
    });

    it('should handle all log parameters', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 200,
        data: 'logs here'
      });

      await getDockerLogs('nginx', { 
        tail: 200, 
        since: '2024-01-15T00:00:00Z',
        timestamps: false 
      });

      expect(callApi).toHaveBeenCalled();
      
      // Check that the path contains the expected query params
      const calledPath = (vi.mocked(callApi).mock.calls[0][0] as { path: string }).path;
      expect(calledPath).toContain('/api/docker/nginx/logs?');
      expect(calledPath).toContain('tail=200');
      expect(calledPath).toContain('timestamps=false');
      // Since parameter gets URL encoded
      expect(calledPath).toContain('since=');
    });

    it('should throw error when container not found', async () => {
      vi.mocked(callApi).mockResolvedValueOnce({
        status: 404,
        data: null
      });

      await expect(getDockerLogs('nonexistent', {})).rejects.toThrow('Container "nonexistent" not found');
    });
  });
});